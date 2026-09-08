use proptest::prelude::*;
use styling_lint::{SpacingOptions, lint};

fn assert_fix(source: &str, expected: &str, options: &SpacingOptions) {
    let result = lint(source, options).unwrap();

    assert_eq!(result.output, expected);
    syn::parse_file(&result.output).unwrap();

    let repeated = lint(&result.output, options).unwrap();

    assert!(
        repeated.diagnostics.is_empty(),
        "{:?}",
        repeated.diagnostics
    );
    assert_eq!(repeated.output, expected);
}

#[test]
fn default_group_boundary_matrix() {
    let statements = ["let x = 1;", "call();", "if true {}"];

    for (previous_index, previous) in statements.iter().enumerate() {
        for (next_index, next) in statements.iter().enumerate() {
            let source = format!("fn example() {{\n    {previous}\n    {next}\n}}\n");
            let grouped = previous_index == next_index && previous_index != 2;
            let expected = format!(
                "fn example() {{\n    {previous}\n{}    {next}\n}}\n",
                if grouped { "" } else { "\n" }
            );

            assert_fix(&source, &expected, &SpacingOptions::default());
            assert_eq!(
                lint(&source, &SpacingOptions::default())
                    .unwrap()
                    .diagnostics
                    .len(),
                usize::from(!grouped)
            );
        }
    }
}

#[test]
fn all_assignment_forms_share_a_group() {
    let source = "fn example() {\nlet x = 1;\nlet Some(x) = value else { return };\nx = 2;\nx += 2;\nx -= 2;\nx *= 2;\nx /= 2;\nx %= 2;\nx ^= 2;\nx &= 2;\nx |= 2;\nx <<= 2;\nx >>= 2;\n}";

    assert_fix(source, source, &SpacingOptions::default());
}

#[test]
fn call_wrappers_and_statement_macros_share_a_group() {
    let source = "async fn example() {\ncall();\nvalue.method();\n(call())?;\n(call().await)?;\nvalue.method()?.await;\ncustom!();\ncustom![];\ncustom! {}\n}";

    assert_fix(source, source, &SpacingOptions::default());
}

#[test]
fn tail_calls_are_separate_but_tail_statement_macros_preserve_source_parity() {
    assert_fix(
        "fn example() {\ncall();\ncall()\n}",
        "fn example() {\ncall();\n\ncall()\n}",
        &SpacingOptions::default(),
    );
    assert_fix(
        "fn example() {\ncall();\ncustom!()\n}",
        "fn example() {\ncall();\n\ncustom!()\n}",
        &SpacingOptions::default(),
    );

    let macro_source = "fn example() {\ncall();\ncustom! {}\n}";

    assert_fix(macro_source, macro_source, &SpacingOptions::default());
}

#[test]
fn preserves_original_fixture() {
    assert_fix(
        "fn example() {\nlet a = 1;\nlet b = 2;\ncall();\ncall()?;\nif a < b {\ncall();\n}\nvalue\n}",
        "fn example() {\nlet a = 1;\nlet b = 2;\n\ncall();\ncall()?;\n\nif a < b {\ncall();\n}\n\nvalue\n}",
        &SpacingOptions::default(),
    );
}

#[test]
fn recurses_into_rust_blocks() {
    let bodies = [
        "fn inner() {\nlet x = 1;\ncall();\n}",
        "let closure = || {\nlet x = 1;\ncall();\n};",
        "let future = async {\nlet x = 1;\ncall();\n};",
        "unsafe {\nlet x = 1;\ncall();\n}",
        "const {\nlet x = 1;\ncall();\n}",
        "loop {\nlet x = 1;\ncall();\n}",
        "match value { _ => {\nlet x = 1;\ncall();\n} }",
        "let Some(x) = value else {\ncall();\nreturn;\n};",
    ];

    for body in bodies {
        let source = format!("fn example() {{\n{body}\n}}");
        let expected = source
            .replace("let x = 1;\ncall();", "let x = 1;\n\ncall();")
            .replace("call();\nreturn;", "call();\n\nreturn;");

        assert_fix(&source, &expected, &SpacingOptions::default());
    }
}

#[test]
fn preserves_opaque_literals_and_macro_tokens() {
    let source = "fn example() {\nlet text = r###\"one\n\n/* two */\nthree\"###;\ncustom! { let x = 1;\ncall(); }\n}";
    let expected = source.replace("\"###;\ncustom", "\"###;\n\ncustom");

    assert_fix(source, &expected, &SpacingOptions::default());
}

#[test]
fn comments_and_attributes_stay_attached() {
    assert_fix(
        "fn example() {\nlet x = 1; // trailing\n// leading\n#[allow(unused)]\ncall();\n}",
        "fn example() {\nlet x = 1; // trailing\n\n// leading\n#[allow(unused)]\ncall();\n}",
        &SpacingOptions::default(),
    );
    assert_fix(
        "fn example() {\nlet x = 1;\n/// documentation\nfn inner() {}\n}",
        "fn example() {\nlet x = 1;\n\n/// documentation\nfn inner() {}\n}",
        &SpacingOptions::default(),
    );
}

#[test]
fn inserts_after_trailing_nested_multiline_comment() {
    assert_fix(
        "fn example() {\nlet x = 1; /* trailing\n\n/* nested */\nend */\n// leading\ncall();\n}",
        "fn example() {\nlet x = 1; /* trailing\n\n/* nested */\nend */\n\n// leading\ncall();\n}",
        &SpacingOptions::default(),
    );
}

#[test]
fn closing_comment_on_next_statement_line_stays_intact() {
    assert_fix(
        "fn example() {\nlet x = 1; /* trailing\nend */ call();\n}",
        "fn example() {\nlet x = 1; /* trailing\nend */\n\n call();\n}",
        &SpacingOptions::default(),
    );
}

#[test]
fn blank_lines_inside_leading_comments_do_not_count() {
    assert_fix(
        "fn example() {\nlet x = 1;\n/* leading\n\nend */\ncall();\n}",
        "fn example() {\nlet x = 1;\n\n/* leading\n\nend */\ncall();\n}",
        &SpacingOptions::default(),
    );
}

#[test]
fn existing_whitespace_only_lines_and_extra_spacing_are_preserved() {
    let source = "fn example() {\nlet x = 1;\n \t\n\n// attached\ncall();\n\n\nreturn;\n}";

    assert_fix(source, source, &SpacingOptions::default());
}

#[test]
fn unicode_columns_and_bom_are_preserved() {
    for prefix in [
        "",
        "\u{feff}",
        "#!/usr/bin/env rust-script\n",
        "\u{feff}#!/usr/bin/env rust-script\n",
    ] {
        let source = format!("{prefix}fn café() {{ let naïve = \"😀\";\n    call();\n}}");
        let expected = source.replace("\";\n", "\";\n\n");

        assert_fix(&source, &expected, &SpacingOptions::default());

        let diagnostic = lint(&source, &SpacingOptions::default())
            .unwrap()
            .diagnostics
            .remove(0);

        assert_eq!(diagnostic.column, 5);
        assert_eq!(diagnostic.line, if prefix.contains('\n') { 3 } else { 2 });
    }
}

#[test]
fn preserves_local_newline_style_and_missing_final_newline() {
    for source in [
        "fn example() {\nlet x = 1;\ncall();\n}",
        "fn example() {\r\nlet x = 1;\r\ncall();\r\n}",
        "fn example() {\r\nlet x = 1;\ncall();\r\nreturn;\n}",
    ] {
        let expected = source
            .replace("let x = 1;\n", "let x = 1;\n\n")
            .replace("let x = 1;\r\n", "let x = 1;\r\n\r\n")
            .replace("call();\r\nreturn", "call();\r\n\r\nreturn");

        assert_fix(source, &expected, &SpacingOptions::default());
    }
}

#[test]
fn grouping_can_be_disabled_independently() {
    for (assignments, calls) in [(false, true), (true, false), (false, false)] {
        let options = SpacingOptions {
            group_assignments: assignments,
            group_calls: calls,
            ..Default::default()
        };
        let source = "fn example() {\nlet a = 1;\nlet b = 2;\n\ncall();\ncall();\n}";
        let mut expected = source.to_owned();

        if !assignments {
            expected = expected.replace("let a = 1;\n", "let a = 1;\n\n");
        }

        if !calls {
            expected = expected.replace("call();\ncall();", "call();\n\ncall();");
        }

        assert_fix(source, &expected, &options);
    }
}

#[test]
fn configurable_minimum_adds_only_missing_lines() {
    let options = SpacingOptions {
        min_blank_lines: 3,
        ..Default::default()
    };
    let source = "fn example() {\nlet x = 1;\n\ncall();\n}";
    let result = lint(source, &options).unwrap();

    assert_eq!(result.diagnostics[0].required, 3);
    assert_eq!(result.diagnostics[0].found, 1);
    assert_fix(
        source,
        "fn example() {\nlet x = 1;\n\n\n\ncall();\n}",
        &options,
    );
}

#[test]
fn same_line_boundaries_and_empty_input_are_unchanged() {
    for source in [
        "",
        "\n",
        "fn example() {}",
        "fn example() { let x = 1; call(); return; }",
    ] {
        assert_fix(source, source, &SpacingOptions::default());
    }
}

#[test]
fn syntax_errors_and_extreme_options_return_errors() {
    assert!(lint("fn broken( {", &SpacingOptions::default()).is_err());
    assert!(
        lint(
            "",
            &SpacingOptions {
                min_blank_lines: usize::MAX,
                ..Default::default()
            }
        )
        .is_err()
    );

    let source = "fn example() {\nlet x = 1;\ncall();\n}";

    assert_fix(
        source,
        source,
        &SpacingOptions {
            min_blank_lines: 0,
            ..Default::default()
        },
    );
}

proptest! {
    #[test]
    fn randomized_layouts_are_parseable_preserving_and_idempotent(
        statements in prop::collection::vec(0usize..6, 1..30),
        blank_lines in 0usize..5,
        minimum in 1usize..=10,
        crlf in any::<bool>(),
        assignments in any::<bool>(),
        calls in any::<bool>(),
    ) {
        let newline = if crlf { "\r\n" } else { "\n" };
        let choices = [
            "let café = r#\"literal\n\n/* content */\"#;",
            "call();",
            "if true {\nlet x = 1;\ncall();\n}",
            "return;",
            "custom! { let x = 1;\ncall(); }",
            "café += 1;",
        ];
        let mut source = String::from("fn example() {\n");
        for (index, choice) in statements.iter().enumerate() {
            source.push_str(choices[*choice]);
            source.push_str(" /* trailing\n\n/* nested */ end */\n");
            source.push_str(&"\n".repeat(blank_lines));
            source.push_str(&format!("// leading {index}\n"));
        }
        source.push('}');
        let source = source.replace('\n', newline);
        let options = SpacingOptions {
            min_blank_lines: minimum,
            group_assignments: assignments,
            group_calls: calls,
        };
        let result = lint(&source, &options).unwrap();
        prop_assert!(syn::parse_file(&result.output).is_ok());
        let repeated = lint(&result.output, &options).unwrap();
        prop_assert!(repeated.diagnostics.is_empty());
        prop_assert_eq!(&repeated.output, &result.output);
        let without_line_breaks = |text: &str| text.chars().filter(|c| *c != '\r' && *c != '\n').collect::<String>();
        prop_assert_eq!(without_line_breaks(&source), without_line_breaks(&result.output));
        let comment = "/* trailing\n\n/* nested */ end */".replace('\n', newline);
        prop_assert_eq!(result.output.matches(&comment).count(), statements.len());
        let literal = "r#\"literal\n\n/* content */\"#".replace('\n', newline);
        prop_assert_eq!(result.output.matches(&literal).count(), statements.iter().filter(|choice| **choice == 0).count());
    }
}
