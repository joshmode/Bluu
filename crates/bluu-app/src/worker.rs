use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc,
};

use bluu_core::{
    backend::BackendError,
    model::{
        CaptureOutcome, CaptureRequest, InboxCodecChange, ProviderOperationOutcome,
        ProviderOperationRequest, Snapshot,
    },
    AudioBackend,
};

#[derive(Debug)]
pub enum WorkerEvent {
    Scanned(Box<Result<Snapshot, BackendError>>),
    Captured(Result<CaptureOutcome, BackendError>),
    ProviderOperated(Box<Result<ProviderOperationOutcome, BackendError>>),
    InboxCodecChanged(Result<InboxCodecChange, BackendError>),
}

#[derive(Debug)]
pub enum WorkerRequest {
    Scan,
    Capture(CaptureRequest),
    ProviderOperation(ProviderOperationRequest),
    SetInboxCodec { aac_enabled: bool, dry_run: bool },
    RestoreInboxCodec,
}

pub struct Worker {
    tx: Sender<WorkerRequest>,
    rx: Receiver<WorkerEvent>,
    scanning: Arc<std::sync::atomic::AtomicBool>,
}

impl Worker {
    pub fn spawn(mut backend: Box<dyn AudioBackend>) -> Self {
        let (req_tx, req_rx) = mpsc::channel::<WorkerRequest>();
        let (event_tx, event_rx) = mpsc::channel::<WorkerEvent>();
        let scanning = Arc::new(std::sync::atomic::AtomicBool::new(false));
        std::thread::Builder::new()
            .name("bluu-backend".into())
            .spawn(move || {
                while let Ok(request) = req_rx.recv() {
                    match request {
                        WorkerRequest::Scan => {
                            let _ = event_tx.send(WorkerEvent::Scanned(Box::new(backend.scan())));
                        }
                        WorkerRequest::Capture(request) => {
                            let _ = event_tx.send(WorkerEvent::Captured(
                                backend.capture_active_codec(&request),
                            ));
                        }
                        WorkerRequest::ProviderOperation(request) => {
                            let _ = event_tx.send(WorkerEvent::ProviderOperated(Box::new(
                                backend.provider_operation(&request),
                            )));
                        }
                        WorkerRequest::SetInboxCodec {
                            aac_enabled,
                            dry_run,
                        } => {
                            let _ = event_tx.send(WorkerEvent::InboxCodecChanged(
                                backend.set_inbox_codec_toggle(aac_enabled, dry_run),
                            ));
                        }
                        WorkerRequest::RestoreInboxCodec => {
                            let _ = event_tx.send(WorkerEvent::InboxCodecChanged(
                                backend.restore_inbox_codec_toggle(),
                            ));
                        }
                    }
                }
            })
            .expect("backend worker thread");
        Self {
            tx: req_tx,
            rx: event_rx,
            scanning,
        }
    }

    pub fn scan(&self) {
        if !self
            .scanning
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            let _ = self.tx.send(WorkerRequest::Scan);
        }
    }

    pub fn scan_finished(&self) {
        self.scanning
            .store(false, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn capture(&self, request: CaptureRequest) {
        let _ = self.tx.send(WorkerRequest::Capture(request));
    }

    pub fn provider_operation(&self, request: ProviderOperationRequest) {
        let _ = self.tx.send(WorkerRequest::ProviderOperation(request));
    }

    pub fn set_inbox_codec(&self, aac_enabled: bool, dry_run: bool) {
        let _ = self.tx.send(WorkerRequest::SetInboxCodec {
            aac_enabled,
            dry_run,
        });
    }

    pub fn restore_inbox_codec(&self) {
        let _ = self.tx.send(WorkerRequest::RestoreInboxCodec);
    }

    pub fn poll(&self) -> Vec<WorkerEvent> {
        self.rx.try_iter().collect()
    }
}
