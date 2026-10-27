use std::sync::{
    mpsc::{self, Receiver, Sender},
    Arc,
};

use bluu_core::{
    backend::BackendError,
    model::{
        CaptureOutcome, CaptureRequest, ProviderOperationOutcome, ProviderOperationRequest,
        Snapshot,
    },
    AudioBackend,
};

#[derive(Debug)]
pub enum WorkerEvent {
    Scanned(Box<Result<Snapshot, BackendError>>),
    Captured(Result<CaptureOutcome, BackendError>),
    ProviderOperated(Box<Result<ProviderOperationOutcome, BackendError>>),
}

#[derive(Debug)]
pub enum WorkerRequest {
    Scan,
    Capture(CaptureRequest),
    ProviderOperation(ProviderOperationRequest),
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

    pub fn poll(&self) -> Vec<WorkerEvent> {
        self.rx.try_iter().collect()
    }
}
