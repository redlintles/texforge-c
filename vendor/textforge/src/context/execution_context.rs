use crate::{
    parser::resolve_var::TokenWrapper,
    utils::errors::{
        TextForgeError,
        TextForgeErrorCode::{self, VariableNotFound},
    },
};
use std::{borrow::Cow, collections::HashMap};
#[derive(Clone)]
pub enum VarValues {
    String(String),
    Usize(usize),
    Token(TokenWrapper),
}

#[cfg(feature = "test_access")]
pub enum ToClean {
    Block(String),
    Var(String),
}

// First thought of a simple hashmap, but it wouldn't suffice my needs
#[allow(dead_code)]
pub struct VarEntry {
    pub value: VarValues,
    pub mutable: bool,
}
// This Object will be re-created every time the program starts.
// Some tokens could access this object for additional data
#[derive(Default)]
pub struct GlobalExecutionContext {
    variables: HashMap<String, VarEntry>,
    blocks: HashMap<String, Vec<TokenWrapper>>,
    replace_maps: HashMap<String, Vec<(std::sync::Arc<regex::Regex>, String)>>,
    pub is_halted: bool,
}

// Variable Concept

// val {name} = {value}; for immutable variables
// var {name} = {value}; for mutable variables
// then the user could reference the variable by ${name} syntax
// And alter it throught mut {name} = {new_value}; instruction if it's mutable.

// Block concept

// blk {name} assoc {instruction};
// if {name} block doesn't exist yet, it's created
// The instruction will be parsed to a Box<dyn InstructionMethods> and then,
// added to the {name} block in the Context's blocks hashmap.
// If the user wish to add multiple instructions to a single block, it should do one per `blk assoc` line.
// Once the user is done with composing a block
// cblk {name}; will execute all instructions stored in the {name} block;

pub trait GlobalContextMethods {
    fn add_to_block(&mut self, block_id: &str, token: TokenWrapper) -> Result<(), TextForgeError>;
    fn get_formatted_block_items(&mut self, block_id: &str) -> Result<String, TextForgeError>;

    fn add_var(&mut self, id: &str, var_entry: VarEntry) -> Result<(), TextForgeError>;
    fn rm_var(&mut self, var_id: &str) -> Result<(), TextForgeError>;
    fn get_var(&self, var_id: &str) -> Result<&VarEntry, TextForgeError>;
    fn get_mut_var(&mut self, var_id: &str) -> Result<&mut VarEntry, TextForgeError>;

    // It would require a more complex implementation. but would help optimizing textforge in the future. This will remove data that will no longer be used from the context.
    #[cfg(feature = "test_access")]
    fn clean_context(&mut self) {}
    fn take_block(&mut self, block_id: &str) -> Result<Vec<TokenWrapper>, TextForgeError>;
    fn put_block(&mut self, block_id: &str, block: Vec<TokenWrapper>);
    fn get_all_vars(&self) -> &HashMap<String, VarEntry>;
    fn halt(&mut self) -> bool;
}

impl GlobalExecutionContext {
    pub(crate) fn define_replace_rule(
        &mut self,
        name: &str,
        regex: std::sync::Arc<regex::Regex>,
        replacement: String,
    ) {
        let rules = self.replace_maps.entry(name.to_owned()).or_default();
        if let Some(rule) = rules
            .iter_mut()
            .find(|(existing, _)| existing.as_str() == regex.as_str())
        {
            rule.1 = replacement;
        } else {
            rules.push((regex, replacement));
        }
    }

    pub(crate) fn execute_replace_map<'a>(
        &self,
        name: &str,
        input: Cow<'a, str>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let rules = self.replace_maps.get(name).ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::ValidationError("Replacement map not found".into()),
                "rmape",
                name.to_owned(),
            )
        })?;
        // All candidates refer to the original input. Re-search from the new
        // cursor when a competing match overlaps: advancing an independent
        // captures_iter could skip valid overlapping occurrences of that rule.
        let mut matches: Vec<_> = rules
            .iter()
            .map(|(regex, _)| regex.captures(&input))
            .collect();
        let mut output = String::new();
        let mut cursor = 0;
        let mut last_end = None;
        loop {
            let mut chosen = None;
            for (index, candidate) in matches.iter_mut().enumerate() {
                if candidate
                    .as_ref()
                    .and_then(|caps| caps.get(0))
                    .is_some_and(|m| {
                        m.start() < cursor || (m.is_empty() && last_end == Some(m.start()))
                    })
                {
                    *candidate = rules[index].0.captures_at(&input, cursor);
                    if candidate
                        .as_ref()
                        .and_then(|caps| caps.get(0))
                        .is_some_and(|m| m.is_empty() && last_end == Some(m.start()))
                    {
                        *candidate = input[cursor..].chars().next().and_then(|c| {
                            rules[index].0.captures_at(&input, cursor + c.len_utf8())
                        });
                    }
                }
                if let Some(m) = candidate.as_ref().and_then(|caps| caps.get(0))
                    && chosen.is_none_or(|(_, start)| m.start() < start)
                {
                    chosen = Some((index, m.start()));
                }
            }
            let Some((index, _)) = chosen else {
                break;
            };
            if let Some(caps) = matches[index].take()
                && let Some(m) = caps.get(0)
            {
                output.push_str(&input[cursor..m.start()]);
                caps.expand(&rules[index].1, &mut output);
                cursor = m.end();
                last_end = Some(cursor);
            }
            let search = if last_end == Some(cursor) && cursor == input.len() {
                None
            } else {
                Some(cursor)
            };
            matches[index] = search.and_then(|at| rules[index].0.captures_at(&input, at));
        }
        drop(matches);
        if last_end.is_none() {
            return Ok(input);
        }
        output.push_str(&input[cursor..]);
        Ok(Cow::Owned(output))
    }

    pub fn new() -> Self {
        GlobalExecutionContext {
            variables: HashMap::new(),
            blocks: HashMap::new(),
            replace_maps: HashMap::new(),
            is_halted: false,
        }
    }
}

impl GlobalContextMethods for GlobalExecutionContext {
    fn get_all_vars(&self) -> &HashMap<String, VarEntry> {
        &self.variables
    }

    fn halt(&mut self) -> bool {
        self.is_halted = true;
        self.is_halted
    }

    fn add_to_block(&mut self, block_id: &str, token: TokenWrapper) -> Result<(), TextForgeError> {
        match self.blocks.get_mut(block_id) {
            Some(tokens) => {
                tokens.push(token);
            }
            None => {
                let block_vec = vec![token];

                self.blocks.insert(block_id.to_string(), block_vec);
            }
        }

        Ok(())
    }

    fn take_block(&mut self, block_id: &str) -> Result<Vec<TokenWrapper>, TextForgeError> {
        self.blocks.remove(block_id).ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::BlockNotFound("Block not found".into()),
                "context.take_block",
                block_id.to_string(),
            )
        })
    }

    fn put_block(&mut self, block_id: &str, block: Vec<TokenWrapper>) {
        self.blocks.insert(block_id.to_string(), block);
    }

    fn get_formatted_block_items(&mut self, block_id: &str) -> Result<String, TextForgeError> {
        use colored::Colorize;

        let block_items = self.blocks.get(block_id).ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::BlockNotFound("Block not found".into()),
                "context.get_formatted_block_items",
                block_id.to_owned(),
            )
        })?;
        let mut result = String::new();

        let len = block_items.len();
        if len == 0 {
            result.push_str("\t\t\t\t(EMPTY BLOCK)\n");
            return Ok(result);
        }

        for (i, token) in block_items.iter().enumerate() {
            let is_last = i + 1 == len;

            let prefix = if is_last {
                if len == 1 {
                    "(BLOCK CREATED): ".green()
                } else {
                    "(BLOCK ALREADY EXISTS) ADDING: ".green()
                }
            } else {
                // sem prefixo para itens antigos
                "".normal()
            };

            result.push_str(&format!(
                "\t\t\t\t{}{}\n",
                prefix,
                token.to_textforge_line().yellow()
            ));
        }

        Ok(result)
    }

    fn add_var(&mut self, id: &str, var_entry: VarEntry) -> Result<(), TextForgeError> {
        self.variables.insert(id.to_string(), var_entry);
        Ok(())
    }
    fn rm_var(&mut self, var_id: &str) -> Result<(), TextForgeError> {
        let var_id_owned = var_id.to_owned();

        self.variables.remove(var_id).ok_or_else(|| {
            TextForgeError::new(
                VariableNotFound(Cow::from(format!(
                    "Could not find var {} to remove",
                    var_id_owned
                ))),
                Cow::from("context.rm_var"),
                Cow::from(var_id_owned),
            )
        })?;

        Ok(())
    }

    fn get_var(&self, var_id: &str) -> Result<&VarEntry, TextForgeError> {
        self.variables.get(var_id).ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::VariableNotFound("Variable not found".into()),
                "get_var",
                var_id.to_string(),
            )
        })
    }

    fn get_mut_var(&mut self, var_id: &str) -> Result<&mut VarEntry, TextForgeError> {
        let v = self.variables.get_mut(var_id).ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::VariableNotFound("Variable not found".into()),
                "get_var",
                var_id.to_string(),
            )
        })?;
        if v.mutable {
            Ok(v)
        } else {
            Err(TextForgeError::new(
                TextForgeErrorCode::NonMutableVariableError("Variable is not mutable".into()),
                "get_mut_var",
                var_id.to_string(),
            ))
        }
    }
}
