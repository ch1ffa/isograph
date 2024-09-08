mod semantic_token_generator;
pub(crate) mod semantic_token_legend;

use crate::{
    lsp_runtime_error::LSPRuntimeResult,
    lsp_state::LSPState,
    row_col_offset::{diff_to_end_of_slice, get_index_from_diff, RowColDiff},
};
use common_lang_types::{Span, TextSource, WithSpan};
use intern::string_key::Intern;
use isograph_compiler::{extract_iso_literals_from_file_content, IsoLiteralExtraction};
use isograph_lang_parser::{parse_iso_literal, IsoLiteralExtractionResult};
use isograph_lang_types::{
    ClientFieldDeclarationWithUnvalidatedDirectives, EntrypointTypeAndField,
};
use lsp_types::{
    request::{Request, SemanticTokensFullRequest},
    SemanticToken, SemanticTokens, SemanticTokensParams, SemanticTokensResult,
};
use semantic_token_generator::SemanticTokenGenerator;
use semantic_token_legend::{semantic_token_method, semantic_token_type};

pub fn on_semantic_token_full_request(
    state: &mut LSPState,
    params: <SemanticTokensFullRequest as Request>::Params,
) -> LSPRuntimeResult<<SemanticTokensFullRequest as Request>::Result> {
    let SemanticTokensParams {
        text_document,
        work_done_progress_params: _,
        partial_result_params: _,
    } = params;

    let file_text = state.text_for(&text_document.uri).expect(
        format!(
            "Retrieving semantic tokens for document {}, which has not been opened before.",
            text_document.uri
        )
        .as_str(),
    );
    let literal_extractions = extract_iso_literals_from_file_content(file_text);
    let mut semantic_tokens = vec![];

    // SemanticTokens are all relative to the start of the previous one, so we have to
    // keep track of the start of the last token that we have pushed onto
    // semantic_tokens
    let mut index_of_last_token = 0;

    // N.B. we are relying on the literal extractions being in order on the page.
    for literal_extraction in literal_extractions {
        let IsoLiteralExtraction {
            iso_literal_text,
            iso_literal_start_index,
            const_export_name,
            ..
        } = literal_extraction;

        let initial_diff =
            diff_to_end_of_slice(&file_text[index_of_last_token..iso_literal_start_index]);

        let file_path = text_document.uri.path().intern();
        let text_source = TextSource {
            path: file_path.into(),
            span: Some(Span::new(
                iso_literal_start_index as u32,
                (iso_literal_start_index + iso_literal_text.len()) as u32,
            )),
        };
        let iso_literal_extraction_result = parse_iso_literal(
            iso_literal_text,
            file_path.into(),
            const_export_name,
            text_source,
        );
        if let Ok(iso_literal_extraction_result) = iso_literal_extraction_result {
            let (new_tokens, total_diff) = iso_literal_parse_result_to_tokens(
                iso_literal_extraction_result,
                iso_literal_text,
                initial_diff,
            );
            semantic_tokens.extend(new_tokens);
            let new_index = get_index_from_diff(
                &file_text[index_of_last_token..iso_literal_start_index],
                total_diff,
            );
            index_of_last_token = new_index + index_of_last_token;
        }
    }
    let result = SemanticTokensResult::Tokens(SemanticTokens {
        data: semantic_tokens,
        result_id: None,
    });
    Ok(Some(result))
}

fn iso_literal_parse_result_to_tokens(
    iso_literal_extraction_result: IsoLiteralExtractionResult,
    iso_literal_text: &str,
    initial_diff: RowColDiff,
) -> (Vec<SemanticToken>, RowColDiff) {
    match iso_literal_extraction_result {
        IsoLiteralExtractionResult::ClientFieldDeclaration(client_field_declaration) => {
            client_field_declaration_to_tokens(
                client_field_declaration,
                iso_literal_text,
                initial_diff,
            )
        }
        IsoLiteralExtractionResult::EntrypointDeclaration(entrypoint_declaration) => {
            entrypoint_declaration_to_tokens(entrypoint_declaration, iso_literal_text, initial_diff)
        }
    }
}

fn client_field_declaration_to_tokens(
    client_field_declaration: WithSpan<ClientFieldDeclarationWithUnvalidatedDirectives>,
    iso_literal_text: &str,
    initial_diff: RowColDiff,
) -> (Vec<SemanticToken>, RowColDiff) {
    let mut semantic_token_generator = SemanticTokenGenerator::new(iso_literal_text, initial_diff);
    semantic_token_generator.generate_semantic_token(
        client_field_declaration.item.parent_type.span,
        semantic_token_type(),
    );
    semantic_token_generator.generate_semantic_token(
        client_field_declaration.item.client_field_name.span,
        semantic_token_method(),
    );
    semantic_token_generator.consume()
}

fn entrypoint_declaration_to_tokens(
    entrypoint_declaration: WithSpan<EntrypointTypeAndField>,
    iso_literal_text: &str,
    initial_diff: RowColDiff,
) -> (Vec<SemanticToken>, RowColDiff) {
    let mut semantic_token_generator = SemanticTokenGenerator::new(iso_literal_text, initial_diff);
    semantic_token_generator.generate_semantic_token(
        entrypoint_declaration.item.parent_type.span,
        semantic_token_type(),
    );
    semantic_token_generator.generate_semantic_token(
        entrypoint_declaration.item.client_field_name.span,
        semantic_token_method(),
    );
    semantic_token_generator.consume()
}