use tokio::sync::watch;

// The build relays directly to Telegram DCs, so the conditional admission gate
// has no Middle-End pool to observe and admission is unconditionally open.
pub(crate) async fn configure_admission_gate(admission_tx: &watch::Sender<bool>) {
    admission_tx.send_replace(true);
}
