use super::*;

/// The JSON object the page was served, as (name, wording) pairs, read back by hand: `"name":"wording"` pairs.
fn words_of(page: &str) -> String {
    let start = page.find(r#"<script id="words" type="application/json">"#).unwrap();
    let rest = &page[start..];
    let open = rest.find('>').unwrap() + 1;
    let close = rest.find("</script>").unwrap();
    rest[open..close].to_owned()
}

/// docs/messages.md, the receive page, row 7: every code the computer answers has a message the page is sent, in
/// every language; every refusal message is a code the computer can answer.
#[test]
fn page_7_every_code_has_its_words_in_every_language() {
    let codes: Vec<&str> = Refusal::ALL.iter().map(|r| r.code()).collect();
    let refusals: Vec<&str> = sent().map(|m| m.as_str()).filter(|n| n.starts_with("receive_refused_")).collect();
    assert_eq!(codes, refusals, "a refusal without a message, or a message no refusal answers");
    for language in LANGUAGES {
        let words = words_of(&page(language));
        for r in Refusal::ALL {
            assert!(r.message().for_users());
            let wording = Note::of(r.message(), []).text(language);
            assert!(words.contains(&format!("\"{}\":{}", r.code(), json_string(&wording))), "{language}: {r:?}");
        }
    }
    assert_eq!(words_of(&page("es")).matches("\"receive_refused_name\":\"nombre de archivo no permitido\"").count(), 1);
}

/// Row 7: the statuses are the ones the page has always been answered with.
#[test]
fn page_7_the_statuses_are_unchanged() {
    let statuses: Vec<u16> = Refusal::ALL.iter().map(|r| r.status()).collect();
    assert_eq!(statuses, [400, 411, 409, 409, 503, 507, 507, 400, 404]);
}

/// Row 8: a phone language oopz lacks, or none, gives the default (English), never a blank page.
#[test]
fn page_8_a_language_oopz_lacks_is_the_default() {
    for accept in [vec!["fr-FR,fr;q=0.9"], vec!["pt-BR"], vec![""], vec![], vec!["*"], vec![",;q=0.5"]] {
        assert_eq!(language(accept.clone()), "en", "{accept:?}");
    }
    let page = page("en");
    assert!(page.contains(r#"<html lang="en">"#));
    assert!(words_of(&page).contains(r#""receive_page_send":"Send""#));
}

/// Row 9: a regional Spanish is Spanish (msgkinds' `Catalog::find`), and the first language the phone lists that
/// oopz has wins.
#[test]
fn page_9_regional_spanish_is_spanish() {
    for accept in ["es-419", "es-MX,es;q=0.9", "ES-es", "fr-FR, es;q=0.8, en;q=0.5"] {
        assert_eq!(language([accept]), "es", "{accept}");
    }
    assert_eq!(language(["fr", "en-GB,es"]), "en", "two header lines, in order");
    let page = page("es");
    assert!(page.contains(r#"<html lang="es">"#));
    assert!(words_of(&page).contains(r#""receive_page_send":"Enviar""#));
}

/// Row 11: a wording can't break out of the page: `</script>`, `<!--`, quotes, backslashes, U+2028 and control
/// characters are escaped (an invented wording).
#[test]
fn page_11_the_words_cant_break_the_page() {
    let wording = "a</script><!--\"b\\c\u{2028}d\u{2029}e\nf&g>";
    assert_eq!(json_string(wording), r#""a\u003c/script\u003e\u003c!--\"b\\c\u2028d\u2029e\u000af\u0026g\u003e""#);
    for language in LANGUAGES {
        let words = words_of(&page(language));
        assert!(!words.contains('<') && !words.contains('\u{2028}'), "{language}");
    }
}

/// Rows 13 and 14: an answer is a code only, nothing the phone sent; the page holds no token (it's served the same to
/// every address, and the token is only in its URL).
#[test]
fn page_13_an_answer_is_a_code_only() {
    for r in Refusal::ALL {
        assert!(r.code().bytes().all(|b| b.is_ascii_lowercase() || b == b'_'), "{r:?}");
    }
    assert!(!PAGE.contains("token"));
}
