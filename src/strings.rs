//! What this client says, in both languages.
//!
//! The generic interface words — Back, Options, Open, Exit — come from `symbian_ui::strings`,
//! because they belong to every application on the handset. What is here is what this one says
//! about *its own* subject: logging in, sending, and the long list of ways a connection to Telegram
//! can fail.
//!
//! # Why the failures are the biggest section
//!
//! Because they are the part a person actually reads. A chat list that works needs three words on a
//! softkey bar; a chat list that will not connect needs to say *which* thing went wrong, and each of
//! these names one — a socket that would not open is a different problem from a server that refused,
//! and both are different from a phone whose clock is wrong.
//!
//! MTProto is implemented here from the specification, so these are not a library's error strings
//! being passed through. Each was written when the failure it names was first seen on the handset.

symbian_ui::strings! {
    // ---- signing in ---------------------------------------------------------------------------
    sign_in = { en: "Sign in", pt: "Entrar" },
    /// The affirmative on a login step, where "Select" would say nothing about what happens next.
    next = { en: "Next", pt: "Avançar" },
    phone_number = { en: "Phone number", pt: "Número de telefone" },
    code = { en: "code", pt: "código" },
    enter_sms_code = { en: "Enter the code sent by SMS", pt: "Digite o código enviado por SMS" },
    /// The tail of "5 digits", counted out beside the field so the length is known before typing.
    digits_suffix = { en: " digits", pt: " dígitos" },

    // ---- the chat ------------------------------------------------------------------------------
    send = { en: "Send", pt: "Enviar" },
    /// Fetch again now, rather than waiting for the next poll.
    refresh = { en: "Refresh", pt: "Atualizar" },
    /// The field label, capitalised. `code` above is the same word lower case, inside a sentence.
    code_field = { en: "Code", pt: "Código" },
    debug_log = { en: "Debug log", pt: "Log de depuração" },

    // ---- what a message is, when it is not words ------------------------------------------------
    //
    // A chat row shows the message's text, and these are what it shows when there is none. They read
    // as a noun rather than a sentence because they sit where a message would: "Photo", not
    // "Sent a photo".
    media_photo = { en: "Photo", pt: "Foto" },
    media_voice = { en: "Voice message", pt: "Mensagem de voz" },
    media_audio = { en: "Audio", pt: "Áudio" },
    media_file = { en: "File", pt: "Arquivo" },
    media_other = { en: "Media", pt: "Mídia" },

    // ---- not connected -------------------------------------------------------------------------
    not_connected_yet = { en: "not connected yet", pt: "ainda não conectado" },
    no_connection = { en: "no connection", pt: "sem conexão" },
    no_network = { en: "no network connection", pt: "sem conexão de rede" },
    /// Shown when the platform is waiting on the user to pick a bearer — the dialog is behind us,
    /// so the message's job is to say where to look rather than to describe a fault.
    look_for_the_dialog = { en: "look for the connection dialog", pt: "procure o diálogo de conexão" },
    could_not_open_socket = { en: "could not open the socket", pt: "não consegui abrir o socket" },
    could_not_reach_server = { en: "could not reach the server", pt: "não consegui alcançar o servidor" },
    server_did_not_answer = { en: "the server did not answer", pt: "o servidor não respondeu" },
    could_not_switch_server = { en: "could not switch server", pt: "não consegui mudar de servidor" },
    server_refused_repeatedly = {
        en: "the server refused the request several times",
        pt: "o servidor recusou o pedido várias vezes",
    },
    could_not_resend = { en: "could not resend the request", pt: "não consegui reenviar o pedido" },
    /// Telegram rejects a request whose timestamp is too far out, and the fix is the phone's clock
    /// rather than anything here — so the message names the clock.
    clock_is_wrong = {
        en: "the phone's clock is badly wrong",
        pt: "o relógio do telefone está muito errado",
    },
    could_not_resend_after_clock = {
        en: "could not resend after adjusting the clock",
        pt: "não consegui reenviar depois de ajustar o relógio",
    },
    unreadable_response = { en: "unreadable response", pt: "resposta ilegível" },
    could_not_decrypt = { en: "could not decrypt", pt: "não consegui decifrar" },
    could_not_compute_key = { en: "could not compute the key", pt: "não consegui calcular a chave" },

    // ---- the handshake -------------------------------------------------------------------------
    //
    // Named separately from the failures above because they happen in a known order, so which one
    // appears says how far the exchange got. That is the whole diagnostic value of having six.
    handshake_unreadable_tl = { en: "handshake: unreadable TL", pt: "handshake: TL ilegível" },
    handshake_no_factor = { en: "handshake: could not factor pq", pt: "handshake: não fatorei o pq" },
    handshake_bad_dh = { en: "handshake: invalid DH parameters", pt: "handshake: parâmetros DH inválidos" },
    handshake_nonce_mismatch = { en: "handshake: nonce mismatch", pt: "handshake: nonce não confere" },
    handshake_key_failed = {
        en: "handshake: key generation failed",
        pt: "handshake: geração da chave falhou",
    },
    handshake_keys_differ = { en: "handshake: keys do not match", pt: "handshake: as chaves não batem" },

    // ---- refused, rather than failed -----------------------------------------------------------
    //
    // Telegram answered and said no. Separate from the failures above because there is something
    // for the user to *do* about each of these, and nothing to do about a socket that would not
    // open.
    wrong_code = {
        en: "Wrong code. Check it and try again",
        pt: "Código incorreto. Verifique e tente de novo",
    },
    unknown_number = {
        en: "Telegram does not recognise that number",
        pt: "Número não reconhecido pelo Telegram",
    },
    /// The one failure that is ours rather than the network's: `api.conf` is missing or wrong. Named
    /// so it is not mistaken for a connection problem and debugged for an hour as one.
    bad_api_id = {
        en: "Configuration error (invalid api_id)",
        pt: "Erro de configuração (api_id inválido)",
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use symbian_ui::Lang;

    #[test]
    fn nothing_was_filled_in_by_copying_the_english() {
        let entries: &[fn() -> &'static str] = &[
            sign_in, next, phone_number, code, enter_sms_code, digits_suffix,
            send, refresh, code_field, debug_log,
            media_photo, media_voice, media_audio, media_file, media_other,
            wrong_code, unknown_number, bad_api_id,
            not_connected_yet, no_connection, no_network, look_for_the_dialog,
            could_not_open_socket, could_not_reach_server, server_did_not_answer,
            could_not_switch_server, server_refused_repeatedly, could_not_resend,
            clock_is_wrong, could_not_resend_after_clock, unreadable_response,
            could_not_decrypt, could_not_compute_key,
            handshake_unreadable_tl, handshake_no_factor, handshake_bad_dh,
            handshake_nonce_mismatch, handshake_key_failed, handshake_keys_differ,
        ];
        symbian_ui::lang::set(Lang::En);
        let en: alloc::vec::Vec<&str> = entries.iter().map(|f| f()).collect();
        symbian_ui::lang::set(Lang::Pt);
        let pt: alloc::vec::Vec<&str> = entries.iter().map(|f| f()).collect();
        symbian_ui::lang::set(Lang::En);
        for (e, p) in en.iter().zip(pt.iter()) {
            assert_ne!(e, p, "the same in both languages: {e:?}");
        }
    }

    #[test]
    fn every_failure_says_a_different_thing() {
        // Six handshake steps and eleven connection faults, and the point of having that many is
        // that *which* one appears tells you how far it got. Two that read alike are two that
        // cannot be told apart on a phone screen — so they are checked for collisions rather than
        // trusted to be distinct because they were written on different days.
        let failures: &[fn() -> &'static str] = &[
            not_connected_yet, no_connection, no_network, look_for_the_dialog,
            could_not_open_socket, could_not_reach_server, server_did_not_answer,
            could_not_switch_server, server_refused_repeatedly, could_not_resend,
            clock_is_wrong, could_not_resend_after_clock, unreadable_response,
            could_not_decrypt, could_not_compute_key,
            handshake_unreadable_tl, handshake_no_factor, handshake_bad_dh,
            handshake_nonce_mismatch, handshake_key_failed, handshake_keys_differ,
        ];
        for l in [Lang::En, Lang::Pt] {
            symbian_ui::lang::set(l);
            let mut seen: alloc::vec::Vec<&str> = failures.iter().map(|f| f()).collect();
            let before = seen.len();
            seen.sort_unstable();
            seen.dedup();
            assert_eq!(seen.len(), before, "{l:?}: two failures read the same");
        }
        symbian_ui::lang::set(Lang::En);
    }
}
