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

    // ---- the chat list ---------------------------------------------------------------------
    //
    // "Telegram" itself is never in this table: it is the product's name, and a name is not
    // translated. It stays a literal in `chats.rs`, `chats_decl.rs`, `login.rs`, `login_decl.rs`
    // and `mvu.rs`.
    /// The title-bar detail while a page is on the wire, and the conversation's too.
    loading = { en: "loading…", pt: "carregando…" },
    no_chats = { en: "No chats", pt: "Nenhuma conversa" },
    /// A peer the dialog list knows by id and not by name. Rare, and it still has to draw a row.
    no_name = { en: "(no name)", pt: "(sem nome)" },

    // ---- the conversation ------------------------------------------------------------------
    /// Composed with the link at the call site: `format!("{} {url}", opening())`.
    opening = { en: "opening", pt: "abrindo" },
    /// The same word about media, where there is no address worth showing.
    opening_ellipsis = { en: "opening…", pt: "abrindo…" },
    refreshing = { en: "refreshing…", pt: "atualizando…" },
    /// Above the newest hundred there is nothing held, so scrolling up further asks for nothing.
    start_of_stored = { en: "start of what is stored", pt: "inicio do que esta guardado" },
    /// `format!("{}: {url}", copied())`.
    copied = { en: "copied", pt: "copiado" },
    nothing_to_copy = { en: "nothing to copy", pt: "nada para copiar" },
    message_copied = { en: "message copied", pt: "mensagem copiada" },
    /// Taking the promise back when the platform's clipboard refuses.
    could_not_copy = { en: "could not copy", pt: "nao foi possivel copiar" },
    /// The empty composer.
    compose_placeholder = { en: "Message…", pt: "Mensagem…" },
    /// Inside a bubble's `[…]` label, where `media_voice` would not fit beside the duration.
    media_voice_short = { en: "Voice", pt: "Voz" },

    // ---- the Options menu ------------------------------------------------------------------
    log_on = { en: "Log: on", pt: "Log: ligado" },
    log_off = { en: "Log: off", pt: "Log: desligado" },

    // ---- signing in, continued -------------------------------------------------------------
    /// The title-bar detail on the phone-number screen, lower case beside the product name.
    title_sign_in = { en: "sign in", pt: "entrar" },
    /// The same, on the password screen — and the field's placeholder.
    password = { en: "password", pt: "senha" },
    two_factor_password = { en: "Two-factor password", pt: "Senha de dois fatores" },
    /// A softkey is a verb: it says what pressing it does, not what the field is doing now.
    show = { en: "Show", pt: "Mostrar" },
    hide = { en: "Hide", pt: "Ocultar" },
    /// `format!("{}{}", enter_the(), "5 digits")` — see `digits_suffix`.
    enter_the = { en: "Enter the ", pt: "Digite os " },
    no_api_id = {
        en: "no api_id: see apps/telegram/api.conf.example",
        pt: "sem api_id: veja apps/telegram/api.conf.example",
    },
    connecting_ellipsis = { en: "connecting…", pt: "conectando…" },
    sending_the_code = { en: "sending the code", pt: "enviando o código" },
    resending_the_code = { en: "resending the code", pt: "reenviando o código" },
    signing_in = { en: "signing in", pt: "entrando" },
    checking_password = { en: "checking the password", pt: "verificando a senha" },
    code_expired = {
        en: "The code expired. Ask for a new one and try again",
        pt: "O código expirou. Solicite um novo e tente de novo",
    },
    wrong_password = { en: "Wrong password", pt: "Senha incorreta" },
    /// `format!("{} {n}{}", too_many_attempts(), seconds_suffix())`. Split rather than interpolated
    /// because `strings!` deliberately does not take arguments — see its header.
    too_many_attempts = { en: "Too many attempts. Wait", pt: "Muitas tentativas. Aguarde" },
    seconds_suffix = { en: " seconds", pt: " segundos" },
    no_account_here = {
        en: "This number has no account. This client cannot create one",
        pt: "Este número não tem conta. Este cliente não pode criar uma",
    },

    // ---- what the driver is doing ----------------------------------------------------------
    //
    // These are the whole of the title-bar status line. Each is a state the driver is *in*, not a
    // fault, which is why none of them is in the failure list below.
    starting = { en: "starting", pt: "iniciando" },
    paused = { en: "paused", pt: "pausado" },
    in_background = { en: "in the background", pt: "em segundo plano" },
    connecting = { en: "connecting", pt: "conectando" },
    connected = { en: "connected", pt: "conectado" },
    reconnecting = { en: "reconnecting", pt: "reconectando" },
    sending = { en: "sending…", pt: "enviando…" },
    deriving_the_key = { en: "deriving the key…", pt: "derivando a chave…" },
    switching_server = { en: "switching server", pt: "mudando de servidor" },

    // ---- what the link is doing ------------------------------------------------------------
    waiting_for_network = { en: "waiting for the network", pt: "aguardando rede" },
    connecting_to_telegram = { en: "connecting to Telegram", pt: "conectando ao Telegram" },
    key_material = { en: "key material", pt: "material da chave" },
    connected_nothing_to_send = { en: "connected, nothing to send", pt: "conectado, sem nada a enviar" },
    choose_access_point = { en: "choose an access point", pt: "escolha um ponto de acesso" },
    /// Deliberately reported: the exponentiation takes seconds, and a status line that goes quiet
    /// for four of them reads as a freeze.
    computing_the_key = { en: "computing the key", pt: "calculando a chave" },
    clock_adjusted = { en: "clock adjusted", pt: "relógio ajustado" },

    // ---- the link gave up ------------------------------------------------------------------
    server_refused_a_message = { en: "the server refused a message", pt: "o servidor recusou uma mensagem" },
    handshake_rejected_exponentiation = {
        en: "the handshake rejected the exponentiation",
        pt: "o handshake recusou a exponenciação",
    },
    worker_thread_failed = { en: "the worker thread failed", pt: "a thread de trabalho falhou" },
    worker_refused_job = {
        en: "the worker thread would not take the job",
        pt: "a thread de trabalho não aceitou o serviço",
    },
    could_not_send_greeting = {
        en: "could not send the transport greeting",
        pt: "não consegui enviar a saudação do transporte",
    },
    read_failed = { en: "read failed", pt: "falha na leitura" },
    write_failed = { en: "write failed", pt: "falha na escrita" },
    server_forgot_this_key = {
        en: "the server no longer knows this auth key",
        pt: "o servidor não conhece mais esta chave",
    },
    server_closed_connection = { en: "the server closed the connection", pt: "o servidor fechou a conexão" },
    connection_failed = { en: "the connection failed", pt: "a conexão falhou" },

    // ---- naming the deepest part of a failure ----------------------------------------------
    framing_error = { en: "framing error", pt: "erro de enquadramento" },
    /// `describe`'s shorter form of `server_forgot_this_key`: the same -404, seen where the sentence
    /// has to sit on one line beside a chat name.
    server_forgot_key = { en: "the server forgot the key", pt: "o servidor esqueceu a chave" },
    server_refused = { en: "the server refused", pt: "o servidor recusou" },
    handshake_crypto_failed = { en: "handshake: crypto failure", pt: "handshake: falha de cripto" },
    handshake_out_of_order = { en: "handshake: response out of order", pt: "handshake: resposta fora de ordem" },
    handshake_no_rsa_key = { en: "handshake: none of our RSA keys", pt: "handshake: nenhuma chave RSA nossa" },
    handshake_server_rejected = {
        en: "handshake: the server rejected the data",
        pt: "handshake: servidor recusou os dados",
    },
    handshake_unknown_dh_prime = { en: "handshake: unknown DH prime", pt: "handshake: primo DH desconhecido" },

    // ---- asking again --------------------------------------------------------------------------
    //
    // The word "download" is not translated anywhere below: it is the word Portuguese uses too, and
    // a `download:` prefix that differed between the two would be a copy the check cannot catch.
    already_refreshing = { en: "already refreshing…", pt: "ja atualizando…" },
    refresh_queued = { en: "refresh queued", pt: "atualizacao na fila" },
    no_peer = { en: "no peer", pt: "sem peer" },
    refresh_no_peer = { en: "refresh: no peer", pt: "refresh: sem peer" },

    // ---- fetching media ------------------------------------------------------------------------
    download_no_media = { en: "download: no media", pt: "download: sem media" },
    sticker_no_decoder = {
        en: "sticker: WebP format, no decoder",
        pt: "sticker: formato WebP, sem decoder",
    },
    download_nothing_to_fetch = { en: "download: nothing to fetch", pt: "download: nada para baixar" },
    from_the_cache = { en: "from the cache", pt: "do cache" },
    audio_unsupported = { en: "audio: not supported yet", pt: "audio: sem suporte ainda" },
    /// Lower case: these compose a sentence — `format!("{}: {kb} KB, {}", file_label(), in_the_cache())`
    /// — rather than standing alone the way `media_file` does in a chat row.
    file_label = { en: "file", pt: "arquivo" },
    in_the_cache = { en: "in the cache", pt: "no cache" },
    no_viewer = { en: "no viewer", pt: "sem visualizador" },
    download_wait_for_previous = {
        en: "download: wait for the previous one",
        pt: "download: aguarde o anterior",
    },
    download_no_driver = { en: "download: no driver", pt: "download: sem driver" },
    connecting_to_media_server = {
        en: "connecting to the media server…",
        pt: "conectando ao servidor da midia…",
    },
    /// Composed at the call site, with and without a byte count: `format!("{}…", downloading())` and
    /// `format!("{} {kb} KB…", downloading())`.
    downloading = { en: "downloading", pt: "baixando" },
    download_queued = { en: "download: queued (link busy)", pt: "download: fila (link ocupado)" },
    download_parse_failed = { en: "download: parse failed", pt: "download: parse falhou" },
    download_no_request = { en: "download: no matching request", pt: "download: sem pedido correspondente" },
    /// `format!("{} ({kb} KB)", file_too_big())`.
    file_too_big = { en: "file too big", pt: "arquivo grande demais" },
    download_interrupted = { en: "download: interrupted", pt: "download: interrompido" },
    /// `format!("{} {dc}…", fetching_from_server())`.
    fetching_from_server = { en: "fetching from server", pt: "buscando no servidor" },
    download_no_route = { en: "download: another server, no route", pt: "download: outro servidor, sem rota" },

    // ---- decoding a photo ----------------------------------------------------------------------
    decode_no_file = { en: "decode: no file to decode", pt: "decode: sem arquivo para decodificar" },
    /// `format!("{}: {}", decode_refused(), e.code())`.
    decode_refused = { en: "decode refused", pt: "decode recusou" },
    decode_failed = { en: "decode failed", pt: "decode falhou" },
    decode_no_pixels = { en: "decode with no pixels", pt: "decode sem pixels" },
    decoder_vanished = { en: "stuck: the decoder vanished", pt: "travou: decoder sumiu" },
    /// The head of the stuck-decoder dump, whose tail is numbers.
    stuck = { en: "stuck", pt: "travou" },

    // ---- opening a link ------------------------------------------------------------------------
    /// `format!("{}: {e:?}", could_not_ask())`.
    could_not_ask = { en: "could not ask", pt: "nao consegui pedir" },
    did_not_open = { en: "did not open", pt: "nao abriu" },
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
            loading, no_chats, no_name,
            opening, opening_ellipsis, refreshing, start_of_stored, copied, nothing_to_copy,
            message_copied, could_not_copy, compose_placeholder, media_voice_short,
            log_on, log_off,
            title_sign_in, password, two_factor_password, show, hide, enter_the, no_api_id,
            connecting_ellipsis, sending_the_code, resending_the_code, signing_in,
            checking_password, code_expired, wrong_password, too_many_attempts, seconds_suffix,
            no_account_here,
            starting, paused, in_background, connecting, connected, reconnecting, sending,
            deriving_the_key, switching_server,
            waiting_for_network, connecting_to_telegram, key_material,
            connected_nothing_to_send, choose_access_point, computing_the_key, clock_adjusted,
            server_refused_a_message, handshake_rejected_exponentiation, worker_thread_failed,
            worker_refused_job, could_not_send_greeting, read_failed, write_failed,
            server_forgot_this_key, server_closed_connection, connection_failed,
            framing_error, server_forgot_key, server_refused, handshake_crypto_failed,
            handshake_out_of_order, handshake_no_rsa_key, handshake_server_rejected,
            handshake_unknown_dh_prime,
            already_refreshing, refresh_queued, no_peer, refresh_no_peer,
            download_no_media, sticker_no_decoder, download_nothing_to_fetch, from_the_cache,
            audio_unsupported, file_label, in_the_cache, no_viewer,
            download_wait_for_previous, download_no_driver, connecting_to_media_server,
            downloading, download_queued, download_parse_failed, download_no_request,
            file_too_big, download_interrupted, fetching_from_server, download_no_route,
            decode_no_file, decode_refused, decode_failed, decode_no_pixels, decoder_vanished,
            stuck, could_not_ask, did_not_open,
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
            // The ones the link reports when it gives up, and the ones `describe` names.
            server_refused_a_message, handshake_rejected_exponentiation, worker_thread_failed,
            worker_refused_job, could_not_send_greeting, read_failed, write_failed,
            server_forgot_this_key, server_closed_connection, connection_failed,
            framing_error, server_forgot_key, server_refused, handshake_crypto_failed,
            handshake_out_of_order, handshake_no_rsa_key, handshake_server_rejected,
            handshake_unknown_dh_prime,
            // And the ones a download reports, which are as numerous and as easy to confuse.
            download_no_media, sticker_no_decoder, download_nothing_to_fetch,
            download_wait_for_previous, download_no_driver, download_queued,
            download_parse_failed, download_no_request, download_interrupted,
            download_no_route, decode_no_file, decode_refused, decode_failed,
            decode_no_pixels, decoder_vanished,
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
