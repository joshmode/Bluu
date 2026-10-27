//! Small, short-lived elevated helper. It starts one ETW trace session, watches the
//! Microsoft-Windows-Bluetooth-BthA2dp provider, writes a minimal JSON result, and exits.
//! It accepts no arbitrary commands and makes no system changes.
//!
//! The ETW provider and event are documented only indirectly (Microsoft's
//! BluetoothStack.wprp). The A2dpStreaming payload field layout is not a supported
//! public contract; Bluu labels it as an observation and never as codec configuration.

#[cfg(not(windows))]
fn main() {
    eprintln!("bluu-etw-helper is a Windows-only component.");
    std::process::exit(2);
}

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let invocation = match bluu_platform::launch::parse_helper_args(&args) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    if !bluu_platform::launch::safe_result_path(&invocation.result_path) {
        eprintln!("refusing a result path outside the user's temporary directory");
        std::process::exit(2);
    }
    let result = windows_impl::run(invocation.duration_seconds);
    let output = match result {
        Ok(result) => HelperJson {
            observation: result.observation,
            captured_events: result.events,
            session_started: true,
            note: result.note,
            error: None,
        },
        Err(e) => HelperJson {
            observation: None,
            captured_events: 0,
            session_started: false,
            note: "No codec observation was produced.".into(),
            error: Some(error_text(&e)),
        },
    };
    if let Ok(json) = serde_json::to_vec(&output) {
        let _ = std::fs::write(&invocation.result_path, json);
    }
}

#[cfg(windows)]
fn error_text(error: &bluu_core::backend::BackendError) -> String {
    match error {
        bluu_core::backend::BackendError::ElevationRequired(s)
        | bluu_core::backend::BackendError::HelperStart(s)
        | bluu_core::backend::BackendError::Platform(s)
        | bluu_core::backend::BackendError::DeviceAccess(s)
        | bluu_core::backend::BackendError::AudioAccess(s)
        | bluu_core::backend::BackendError::Unsupported(s)
        | bluu_core::backend::BackendError::InvalidInput(s)
        | bluu_core::backend::BackendError::Provider(s) => s.clone(),
        bluu_core::backend::BackendError::Cancelled => "cancelled".into(),
    }
}

#[cfg(windows)]
#[derive(serde::Serialize)]
struct HelperJson {
    observation: Option<bluu_core::model::ActiveCodecObservation>,
    captured_events: u32,
    session_started: bool,
    note: String,
    error: Option<String>,
}

#[cfg(windows)]
mod windows_impl {
    use std::sync::{
        atomic::{AtomicU32, Ordering},
        Arc, Mutex,
    };
    use std::time::{Duration, Instant};

    use bluu_core::{
        backend::BackendError,
        codec::{Confidence, VerificationMethod},
        model::ActiveCodecObservation,
    };
    use ferrisetw::{
        parser::Parser,
        provider::Provider,
        schema_locator::SchemaLocator,
        trace::{TraceTrait, UserTrace},
        EventRecord,
    };

    const PROVIDER_GUID: &str = "8776AD1E-5022-4451-A566-F47E708B9075";

    pub struct RunResult {
        pub observation: Option<ActiveCodecObservation>,
        pub events: u32,
        pub note: String,
    }

    #[derive(Clone, Copy, Debug, Default)]
    struct CodecEvent {
        standard: u8,
        vendor: u32,
        vendor_codec: u16,
    }

    pub fn run(seconds: u32) -> Result<RunResult, BackendError> {
        let events = Arc::new(AtomicU32::new(0));
        let codec = Arc::new(Mutex::new(None::<CodecEvent>));
        let callback_events = events.clone();
        let callback_codec = codec.clone();
        let provider = Provider::by_guid(PROVIDER_GUID)
            .add_callback(move |record: &EventRecord, locator: &SchemaLocator| {
                callback_events.fetch_add(1, Ordering::Relaxed);
                let Ok(schema) = locator.event_schema(record) else {
                    return;
                };
                // Event IDs and task names are not a stable documented contract across
                // Windows releases. Accept any event that exposes the three known codec
                // fields, and log only the IDs for diagnostics.
                if !schema.task_name().to_ascii_lowercase().contains("a2dp") {
                    return;
                }
                let parser = Parser::create(record, &schema);
                let standard = parser.try_parse::<u8>("A2dpStandardCodecId");
                let Ok(standard) = standard else {
                    return;
                };
                let vendor = parser.try_parse::<u32>("A2dpVendorId").unwrap_or(0);
                let vendor_codec = parser.try_parse::<u16>("A2dpVendorCodecId").unwrap_or(0);
                if let Ok(mut slot) = callback_codec.lock() {
                    *slot = Some(CodecEvent {
                        standard,
                        vendor,
                        vendor_codec,
                    });
                }
            })
            .build();
        let trace = UserTrace::new()
            .named("Bluu-A2DP-Verify".into())
            .enable(provider)
            .start()
            .map_err(|e| BackendError::HelperStart(format!("ETW session: {e:?}")))?;
        let (trace, handle) = trace;
        let started = Instant::now();
        let thread = std::thread::spawn(move || {
            let _ = UserTrace::process_from_handle(handle);
        });
        while started.elapsed() < Duration::from_secs(seconds as u64) {
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = trace.stop();
        let _ = thread.join();
        let count = events.load(Ordering::Relaxed);
        let codec = codec.lock().ok().and_then(|slot| *slot);
        let observation = codec.map(|event| {
            let known = CodecEvent::known_tuple(event);
            let codec = bluu_core::CodecId::from_a2dp_ids(
                event.standard,
                (event.standard == 0xFF).then_some(event.vendor),
                (event.standard == 0xFF).then_some(event.vendor_codec),
            );
            ActiveCodecObservation {
                codec,
                vendor_id: (event.standard == 0xFF).then_some(event.vendor),
                vendor_codec_id: (event.standard == 0xFF).then_some(event.vendor_codec),
                observed_at: chrono::Utc::now(),
                method: VerificationMethod::A2dpEtwEvent,
                device_id: None,
                device_association_confidence: Confidence::Low,
                note: if known {
                    "Observed from Microsoft-Windows-Bluetooth-BthA2dp. Windows does not expose a device identifier in this event payload; confirm with an A2DP protocol capture before treating it as device-specific.".into()
                } else {
                    format!(
                        "The A2DP trace reported an unrecognised standard/vendor tuple ({:#04x}/{:#06x}/{:#06x}). Raw IDs are preserved; no codec name is inferred.",
                        event.standard, event.vendor, event.vendor_codec
                    )
                },
            }
        });
        Ok(RunResult {
            observation,
            events: count,
            note: format!(
                "Watched the A2DP ETW provider for {seconds} s and captured {count} event(s). Play audio during the capture to provoke a stream event."
            ),
        })
    }

    impl CodecEvent {
        fn known_tuple(self) -> bool {
            let codec = bluu_core::CodecId::from_a2dp_ids(
                self.standard,
                (self.standard == 0xFF).then_some(self.vendor),
                (self.standard == 0xFF).then_some(self.vendor_codec),
            );
            codec != bluu_core::CodecId::Other
        }
    }
}
