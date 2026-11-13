//! Testes do front matter: formato válido, erros de forma e as regras do nome.

use super::*;

#[test]
fn a_plain_front_matter_gives_name_and_description() {
    let text = "---\nname: pdf-tools\ndescription: Lê e preenche PDFs.\n---\n# Corpo\n";
    let front = parse(text).expect("valid");
    assert_eq!(front.name, "pdf-tools");
    assert_eq!(front.description, "Lê e preenche PDFs.");
}

#[test]
fn quoted_values_lose_their_quotes_and_escapes() {
    let text = "---\nname: \"review\"\ndescription: 'Diz \"oi\" e it''s ok'\n---\n";
    let front = parse(text).expect("valid");
    assert_eq!(front.name, "review");
    assert_eq!(front.description, "Diz \"oi\" e it's ok");
}

#[test]
fn folded_and_literal_blocks_are_joined() {
    let folded = "---\nname: a\ndescription: >-\n  primeira parte\n  segunda parte\n---\n";
    assert_eq!(parse(folded).unwrap().description, "primeira parte segunda parte");
    let literal = "---\nname: a\ndescription: |\n  linha um\n  linha dois\n---\n";
    assert_eq!(parse(literal).unwrap().description, "linha um\nlinha dois");
}

#[test]
fn comments_and_unknown_keys_are_ignored() {
    let text = "---\n# nota\nname: a\nlicense: MIT\ndescription: ok\n---\n";
    assert!(parse(text).is_ok());
}

#[test]
fn a_byte_order_mark_is_accepted() {
    let text = "\u{feff}---\nname: a\ndescription: ok\n---\n";
    assert!(parse(text).is_ok());
}

#[test]
fn a_file_without_front_matter_is_missing() {
    assert_eq!(parse("# só markdown\n"), Err(FrontError::Missing));
}

#[test]
fn an_unclosed_block_is_unterminated() {
    let text = "---\nname: a\ndescription: ok\n";
    assert_eq!(parse(text), Err(FrontError::Unterminated));
}

#[test]
fn a_line_without_a_colon_reports_its_line_number() {
    let text = "---\nname: a\nisto não é uma chave\n---\n";
    assert_eq!(parse(text), Err(FrontError::BadLine(3)));
}

#[test]
fn a_stray_indented_line_is_rejected() {
    let text = "---\n  solto: x\n---\n";
    assert!(matches!(parse(text), Err(FrontError::BadLine(_))));
}

#[test]
fn name_and_description_are_required() {
    assert_eq!(parse("---\ndescription: ok\n---\n"), Err(FrontError::NameMissing));
    assert_eq!(parse("---\nname: a\n---\n"), Err(FrontError::DescriptionMissing));
    assert_eq!(parse("---\nname: a\ndescription:   \n---\n"), Err(FrontError::DescriptionMissing));
}

#[test]
fn names_follow_the_format_rules() {
    assert!(valid_name("a"));
    assert!(valid_name("pdf-tools-2"));
    assert!(valid_name(&"a".repeat(64)));
    assert!(!valid_name(""));
    assert!(!valid_name(&"a".repeat(65)));
    assert!(!valid_name("PDF"));
    assert!(!valid_name("pdf_tools"));
    assert!(!valid_name("-pdf"));
    assert!(!valid_name("pdf-"));
    assert!(!valid_name("pdf--tools"));
    assert!(!valid_name("pdf tools"));
    assert!(!valid_name("ação"));
}

#[test]
fn an_invalid_name_is_reported_as_such() {
    assert_eq!(parse("---\nname: Bad_Name\ndescription: ok\n---\n"), Err(FrontError::NameInvalid));
}

#[test]
fn every_error_has_its_own_key() {
    let keys = [
        FrontError::Missing.key(),
        FrontError::Unterminated.key(),
        FrontError::BadLine(1).key(),
        FrontError::NameMissing.key(),
        FrontError::NameInvalid.key(),
        FrontError::DescriptionMissing.key(),
    ];
    let unique: std::collections::BTreeSet<_> = keys.iter().collect();
    assert_eq!(unique.len(), keys.len());
}
