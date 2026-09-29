use tokio::sync::watch;

// The build relays directly to Telegram DCs with no upstream pool to
// observe, so the admission gate is unconditionally open.
pub(crate) async fn configure_admission_gate(admission_tx: &watch::Sender<bool>) {
    admission_tx.send_replace(true);
}
