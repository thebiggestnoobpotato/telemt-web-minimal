use super::*;

pub(super) fn get_dc_addr_static(dc_idx: i16, config: &ProxyConfig) -> Result<SocketAddr> {
    let prefer_v6 =
        config.general.network_prefer == 6 && config.general.network_ipv6.unwrap_or(true);
    let datacenters = if prefer_v6 {
        &*TG_DATACENTERS_V6
    } else {
        &*TG_DATACENTERS_V4
    };

    let num_dcs = datacenters.len();

    let dc_key = dc_idx.to_string();
    if let Some(addrs) = config.general.dc_overrides.get(&dc_key) {
        let mut parsed = Vec::new();
        for addr_str in addrs {
            match addr_str.parse::<SocketAddr>() {
                Ok(addr) => parsed.push(addr),
                Err(_) => {
                    warn!(dc_idx = dc_idx, addr_str = %addr_str, "Invalid DC override address in config, ignoring")
                }
            }
        }

        if let Some(addr) = parsed
            .iter()
            .find(|a| a.is_ipv6() == prefer_v6)
            .or_else(|| parsed.first())
            .copied()
        {
            debug!(dc_idx = dc_idx, addr = %addr, count = parsed.len(), "Using DC override from config");
            return Ok(addr);
        }
    }

    let abs_dc = dc_idx.unsigned_abs() as usize;
    if abs_dc >= 1 && abs_dc <= num_dcs {
        return Ok(SocketAddr::new(datacenters[abs_dc - 1], TG_DATACENTER_PORT));
    }

    // Unknown DC requested by client without override: log and fall back.
    if !config.general.dc_overrides.contains_key(&dc_key) {
        warn!(
            dc_idx = dc_idx,
            "Requested non-standard DC with no override; falling back to default cluster"
        );
        // Record each distinct unknown DC index once in the main log
        // destination; the switch gates both the record and the dedup slot.
        if config.logging.unknown_dc_log_enabled
            && should_log_unknown_dc(dc_idx)
        {
            warn!(dc_idx = dc_idx, "unknown_dc");
        }
    }

    let default_dc = config.general.default_dc.unwrap_or(2) as usize;
    let fallback_idx = if default_dc >= 1 && default_dc <= num_dcs {
        default_dc - 1
    } else {
        0
    };

    info!(
        original_dc = dc_idx,
        fallback_dc = (fallback_idx + 1) as u16,
        fallback_addr = %datacenters[fallback_idx],
        "Special DC ---> default_cluster"
    );

    Ok(SocketAddr::new(
        datacenters[fallback_idx],
        TG_DATACENTER_PORT,
    ))
}

pub(super) async fn do_tg_handshake_static<S>(
    mut stream: S,
    success: &HandshakeSuccess,
    config: &ProxyConfig,
    rng: &SecureRandom,
) -> Result<(CryptoReader<ReadHalf<S>>, CryptoWriter<WriteHalf<S>>)>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (nonce, _tg_enc_key, _tg_enc_iv, _tg_dec_key, _tg_dec_iv) = generate_tg_nonce(
        success.proto_tag,
        success.dc_idx,
        &success.enc_key,
        success.enc_iv,
        rng,
        config.general.fast_mode,
    );

    let (encrypted_nonce, tg_encryptor, tg_decryptor) = encrypt_tg_nonce_with_ciphers(&nonce);

    debug!(
        peer = %success.peer,
        nonce_head = %hex::encode(&nonce[..16]),
        "Sending nonce to Telegram"
    );

    stream.write_all(&encrypted_nonce).await?;
    stream.flush().await?;

    let (read_half, write_half) = split(stream);

    let max_pending = config.general.crypto_pending_buffer;
    Ok((
        CryptoReader::new(read_half, tg_decryptor),
        CryptoWriter::new(write_half, tg_encryptor, max_pending),
    ))
}
