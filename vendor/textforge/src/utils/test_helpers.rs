use crate::api::{TextForgeBlockMethods, TextForgeConditionalMethods};
use crate::api::{TextForgeBuilderMethods, processor::TextForgeProcessor};
use crate::utils::errors::TextForgeError;

pub fn build_all_tokens_pipeline_safe(
    processor: &mut TextForgeProcessor,
) -> Result<String, TextForgeError> {
    let id = processor
        .create_pipeline()
        .normalize_unicode("nfc")?
        .add_replace_rule("safe", "NEVER_MATCH_SENTINEL", "replacement")?
        .execute_replace_map("safe")?
        // ========== FASE 0: base previsível e grande ==========
        // Garante B repetido e bastante comprimento desde o início.
        .add_to_beginning("B__SEED__B__SEED__B__SEED__ ")?
        // garante split_select("B", 1) seguro
        .add_to_end(" Banana Laranja cheia de canja ")?
        // texto original
        .pad_right("x", 160)?
        // len alto para índices
        // ========== BLOCO 1: deletes/replace/insert/rotates/trims ==========
        .repeat(3)?
        .delete_after(120)?
        .delete_before(3)?
        .delete_chunk(0, 3)?
        // aqui ainda é string “normal”, mas seu token existe: ok
        .delete_first()?
        .delete_last()?
        .replace_all_with(r"a", "e")?
        .replace_first_with("L", "coxinha")?
        .replace_count_with("e", "carro", 3)?
        .insert(0, "Coxinha Banana")?
        .rotate_left(1)?
        .rotate_right(2)?
        .trim_both_sides()?
        .trim_left_side()?
        .trim_right_side()?
        // Checkpoint: depois de reduzir/alterar, re-garantir comprimento
        .add_to_end(" B__GUARD__B__GUARD__B__GUARD__ ")?
        // mantém Bs
        .pad_right("p", 140)?
        // ========== BLOCO 2: select + caps + split_select ==========
        .add_to_beginning("laranjadebananavermelha")?
        .select(0, 60)?
        // (mudança importante) janela maior e previsível
        .replace_count_with("a", "b", 3)?
        .to_uppercase_all()?
        .to_lowercase_all()?
        .to_uppercase_single(3)?
        .to_lowercase_single(2)?
        .capitalize_first_word()?
        .capitalize_single_word(1)?
        .capitalize_last_word()?
        .capitalize_range(1, 3)?
        // split_select: agora é seguro porque forçamos vários "B" no começo/fim
        .split_select("B", 1)?
        // não deve virar "nj" minúsculo aqui
        // Checkpoint pós split_select (porque ele pode encolher muito)
        .add_to_end(" SAFE_SEGMENT_ABCDEFGHIJKLMNOPQRSTUVWXYZ_0123456789 ")?
        .pad_right("q", 120)?
        // ========== BLOCO 3: replace nth/last + url/html/json ==========
        .capitalize_chunk(1, 3)?
        // ainda existe, mas chunk ainda não foi criado -> seu token aceita? ok.
        .replace_last_with("b", "c")?
        .replace_nth_with("b", "d", 1)?
        // (mudança importante) nth menor pra não depender de muitas ocorrências
        .to_url_encoded()?
        .to_url_decoded()?
        .to_reverse()?
        .split_characters()?
        // daqui em diante estamos em chunks
        .to_html_escaped()?
        .to_html_unescaped()?
        .to_json_escaped()?
        .to_json_unescaped()?
        // Checkpoint: garantir chunk suficiente (split_characters já cria muitos)
        .insert(1, "banana")?
        // insere item/trecho em posição baixa
        .to_uppercase_chunk(0, 6)?
        // ranges curtos e seguros
        .to_lowercase_chunk(0, 10)?
        // ========== BLOCO 4: joins + pads ==========
        .join_to_camel_case()?
        .join_to_kebab_case()?
        .join_to_pascal_case()?
        .join_to_snake_case()?
        .pad_left("xy", 60)?
        .pad_right("yx", 80)?
        // ========== BLOCO 5: if_do_contains_each ==========
        // Condição sempre verdadeira porque pad_left colocou "xy" e pad_right colocou "yx"
        .if_do_contains_each("xy", |b| {
            b.add_to_beginning("Banana")?
                .add_to_end("Bonanza")?
                .capitalize_first_word()?
                .capitalize_last_word()?
                .delete_first()?;
            Ok(())
        })?
        .block_assoc("x", |b| {
            b.add_to_beginning("laranja")?
                .delete_first()?
                .replace_all_with("anja", "anjo")?
                .to_uppercase_chunk(0, 40)?
                .if_do_contains_each("Q", |cb| {
                    cb.replace_all_with("_", "!!!!!!!!!")?;
                    Ok(())
                })?;

            Ok(())
        })?
        .call_block("x")?
        .call_block("x")?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .append_line("b")?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .delete_line(1)?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .delete_lines_after(1)?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .delete_lines_before(2)?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .select_lines(2, 2)?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .delete_lines_interval(2, 2)?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .insert_line(2, "x")?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .remove_line_breaks()?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .replace_line(2, "x")?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .prepend_line("b")?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .delete_empty_lines()?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .keep_lines_matching("^b$")?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .delete_lines_matching("^b$")?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .reverse_lines()?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .unique_lines()?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .join_lines(" ")?
        .build()?;

    Ok(id)
}
