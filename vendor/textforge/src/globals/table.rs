use std::{
    borrow::Cow,
    collections::HashMap,
    sync::{Arc, LazyLock},
};

use crate::{
    tokens::InstructionMethods,
    utils::errors::{TextForgeError, TextForgeErrorCode},
};

use crate::tokens::{instructions::*, transforms::*};

#[derive(Clone)]
pub enum TokenRef {
    #[cfg(feature = "test_access")]
    Boxed(Box<dyn InstructionMethods>),
    Shared(Arc<dyn InstructionMethods>),
}

impl TokenRef {
    fn clone_ref(&self) -> TokenRef {
        match self {
            #[cfg(feature = "test_access")]
            TokenRef::Boxed(b) => TokenRef::Boxed(b.clone()),
            TokenRef::Shared(a) => TokenRef::Shared(a.clone()),
        }
    }

    pub fn into_box(self) -> Box<dyn InstructionMethods> {
        match self {
            #[cfg(feature = "test_access")]
            TokenRef::Boxed(b) => b,
            TokenRef::Shared(a) => a.clone_box(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum QuerySource {
    Identifier(Cow<'static, str>),
    Bytecode(u32),
}

#[derive(Clone, Copy, Debug)]
pub enum QueryTarget {
    #[cfg(feature = "test_access")]
    Identifier,
    #[cfg(feature = "test_access")]
    Bytecode,
    Token,
    Syntax,
}

#[derive(Clone)]
pub enum TargetValue {
    #[cfg(feature = "test_access")]
    Identifier(&'static str),
    #[cfg(feature = "test_access")]
    Bytecode(u32),
    Token(TokenRef),
    Syntax(Arc<[SyntaxDef]>),
}

pub struct TokenTable {
    #[cfg(feature = "test_access")]
    id_to_code: HashMap<&'static str, u32>,
    #[cfg(feature = "test_access")]
    code_to_id: HashMap<u32, &'static str>,
    id_to_token: HashMap<&'static str, TokenRef>,
    code_to_token: HashMap<u32, TokenRef>,
    id_to_syntax: HashMap<&'static str, Arc<[SyntaxDef]>>,
    code_to_syntax: HashMap<u32, Arc<[SyntaxDef]>>,
}

impl TokenTable {
    pub fn find(
        &self,
        (query_source, query_target): (QuerySource, QueryTarget),
    ) -> Result<TargetValue, TextForgeError> {
        let err = || {
            TextForgeError::new(
                TextForgeErrorCode::TokenNotFound("Token Not Found in mapping".into()),
                "TOKEN_TABLE.find()",
                "query",
            )
        };

        match (query_source, query_target) {
            // ✅ CORINGA: QuerySource e QueryTarget "iguais" (eco + valida existência)
            #[cfg(feature = "test_access")]
            (QuerySource::Identifier(id), QueryTarget::Identifier) => {
                // valida que existe
                if self.id_to_code.contains_key(id.as_ref()) {
                    // devolve o próprio id, mas como &'static str:
                    // como seu mapa usa &'static str, pegamos pelo code_to_id via code:
                    let code = *self.id_to_code.get(id.as_ref()).ok_or_else(err)?;
                    let real_id = *self.code_to_id.get(&code).ok_or_else(err)?;
                    Ok(TargetValue::Identifier(real_id))
                } else {
                    Err(err())
                }
            }
            #[cfg(feature = "test_access")]
            (QuerySource::Bytecode(code), QueryTarget::Bytecode) => {
                // valida que existe
                if self.code_to_id.contains_key(&code) {
                    Ok(TargetValue::Bytecode(code))
                } else {
                    Err(err())
                }
            }

            // --- demais casos normais ---
            #[cfg(feature = "test_access")]
            (QuerySource::Identifier(id), QueryTarget::Bytecode) => {
                let code = *self.id_to_code.get(id.as_ref()).ok_or_else(err)?;
                Ok(TargetValue::Bytecode(code))
            }
            (QuerySource::Identifier(id), QueryTarget::Token) => {
                let tok = self.id_to_token.get(id.as_ref()).ok_or_else(err)?;
                Ok(TargetValue::Token(tok.clone_ref()))
            }
            #[cfg(feature = "test_access")]
            (QuerySource::Bytecode(code), QueryTarget::Identifier) => {
                let id = *self.code_to_id.get(&code).ok_or_else(err)?;
                Ok(TargetValue::Identifier(id))
            }
            (QuerySource::Bytecode(code), QueryTarget::Token) => {
                let tok = self.code_to_token.get(&code).ok_or_else(err)?;
                Ok(TargetValue::Token(tok.clone_ref()))
            }
            (QuerySource::Identifier(id), QueryTarget::Syntax) => {
                let params = self.id_to_syntax.get(id.as_ref()).ok_or_else(err)?;
                Ok(TargetValue::Syntax(params.clone()))
            }
            (QuerySource::Bytecode(code), QueryTarget::Syntax) => {
                let params = self.code_to_syntax.get(&code).ok_or_else(err)?;
                Ok(TargetValue::Syntax(params.clone()))
            }
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntaxToken {
    String,
    Usize,
    Token,
    Literal(&'static str),
}

pub struct SyntaxDef {
    pub token: SyntaxToken,
    pub optional: bool,
}

impl SyntaxDef {
    pub fn opt(token: SyntaxToken) -> Self {
        SyntaxDef {
            token,
            optional: true,
        }
    }
    pub fn req(token: SyntaxToken) -> Self {
        SyntaxDef {
            token,
            optional: false,
        }
    }
}

macro_rules! define_token_table {
    (
        $vis:vis static $name:ident = [
            $((
                $id:literal,
                $code:expr,
                $ctor:expr,
                $params:expr $(,)? // ✅ aceita vírgula opcional aqui
            )),*
            $(,)?
        ];
    ) => {
        $vis static $name: LazyLock<TokenTable> = LazyLock::new(|| {
            let mut id_to_code: HashMap<&'static str, u32> = HashMap::new();
            let mut code_to_id: HashMap<u32, &'static str> = HashMap::new();
            let mut id_to_token: HashMap<&'static str, TokenRef> = HashMap::new();
            let mut code_to_token: HashMap<u32, TokenRef> = HashMap::new();

            let mut id_to_syntax: HashMap<&'static str, Arc<[SyntaxDef]>> = HashMap::new();
            let mut code_to_syntax: HashMap<u32, Arc<[SyntaxDef]>> = HashMap::new();

            $(
                let token: TokenRef = ($ctor)();
                let token_for_code = token.clone_ref();

                if id_to_code.contains_key($id) {
                    panic!("define_token_table: duplicate identifier: {}", $id);
                }
                if code_to_id.contains_key(&$code) {
                    panic!("define_token_table: duplicate bytecode: 0x{:x} for {}", $code, $id);
                }

                // ✅ aqui é o ponto crítico: array -> slice

                id_to_code.insert($id, $code);
                code_to_id.insert($code, $id);

                id_to_token.insert($id, token.clone_ref());
                code_to_token.insert($code, token_for_code);

                let arc_params = Arc::new($params);

                id_to_syntax.insert($id, arc_params.clone());
                code_to_syntax.insert($code, arc_params);
            )*

            TokenTable {
                #[cfg(feature = "test_access")]
                id_to_code,
                #[cfg(feature = "test_access")]
                code_to_id,
                id_to_token,
                code_to_token,
                id_to_syntax,
                code_to_syntax,
            }
        });
    };
}

// ✅ sua “tabela 3 colunas” vira só isso:
define_token_table! {
    pub static TOKEN_TABLE = [
        (
            "atb",
            0x01,
            || TokenRef::Shared(Arc::new(atb::Atb::default())),
            [SyntaxDef::req(SyntaxToken::String)],
        ),
        (
            "ate",
            0x02,
            || TokenRef::Shared(Arc::new(ate::Ate::default())),
            [SyntaxDef::req(SyntaxToken::String)],
        ),
        (
            "dlc",
            0x08,
            || TokenRef::Shared(Arc::new(dlc::Dlc::default())),
            [SyntaxDef::req(SyntaxToken::Usize), SyntaxDef::req(SyntaxToken::Usize)],
        ),
        ("dlf", 0x03, || TokenRef::Shared(Arc::new(dlf::Dlf::default())), []),
        ("dll", 0x04, || TokenRef::Shared(Arc::new(dll::Dll::default())), []),
        (
            "dla",
            0x09,
            || TokenRef::Shared(Arc::new(dla::Dla::default())),
            [SyntaxDef::req(SyntaxToken::Usize)],
        ),
        (
            "dlb",
            0x0a,
            || TokenRef::Shared(Arc::new(dlb::Dlb::default())),
            [SyntaxDef::req(SyntaxToken::Usize)],
        ),
        (
            "rfw",
            0x0b,
            || TokenRef::Shared(Arc::new(rfw::Rfw::default())),
            [SyntaxDef::req(SyntaxToken::String), SyntaxDef::req(SyntaxToken::String)],
        ),
        (
            "rcw",
            0x10,
            || TokenRef::Shared(Arc::new(rcw::Rcw::default())),
            [
                SyntaxDef::req(SyntaxToken::String),
                SyntaxDef::req(SyntaxToken::String),
                SyntaxDef::req(SyntaxToken::Usize),
            ],
        ),
        (
            "raw",
            0x0c,
            || TokenRef::Shared(Arc::new(raw::Raw::default())),
            [SyntaxDef::req(SyntaxToken::String), SyntaxDef::req(SyntaxToken::String)],
        ),
        ("tbs", 0x05, || TokenRef::Shared(Arc::new(tbs::Tbs::default())), []),
        ("tls", 0x06, || TokenRef::Shared(Arc::new(tls::Tls::default())), []),
        ("trs", 0x07, || TokenRef::Shared(Arc::new(trs::Trs::default())), []),
        (
            "rpt",
            0x0d,
            || TokenRef::Shared(Arc::new(rpt::Rpt::default())),
            [SyntaxDef::req(SyntaxToken::Usize)],
        ),
        (
            "rtr",
            0x0f,
            || TokenRef::Shared(Arc::new(rtr::Rtr::default())),
            [SyntaxDef::req(SyntaxToken::Usize)],
        ),
        (
            "rtl",
            0x0e,
            || TokenRef::Shared(Arc::new(rtl::Rtl::default())),
            [SyntaxDef::req(SyntaxToken::Usize)],
        ),
        (
            "slt",
            0x11,
            || TokenRef::Shared(Arc::new(slt::Slt::default())),
            [SyntaxDef::req(SyntaxToken::Usize), SyntaxDef::req(SyntaxToken::Usize)],
        ),
        ("tua", 0x12, || TokenRef::Shared(Arc::new(tua::Tua::default())), []),
        ("tla", 0x13, || TokenRef::Shared(Arc::new(tla::Tla::default())), []),
        (
            "tucs",
            0x14,
            || TokenRef::Shared(Arc::new(tucs::Tucs::default())),
            [SyntaxDef::req(SyntaxToken::Usize)],
        ),
        (
            "tlcs",
            0x15,
            || TokenRef::Shared(Arc::new(tlcs::Tlcs::default())),
            [SyntaxDef::req(SyntaxToken::Usize)],
        ),
        (
            "tucc",
            0x16,
            || TokenRef::Shared(Arc::new(tucc::Tucc::default())),
            [SyntaxDef::req(SyntaxToken::Usize), SyntaxDef::req(SyntaxToken::Usize)],
        ),
        (
            "tlcc",
            0x17,
            || TokenRef::Shared(Arc::new(tlcc::Tlcc::default())),
            [SyntaxDef::req(SyntaxToken::Usize), SyntaxDef::req(SyntaxToken::Usize)],
        ),
        ("cfw", 0x18, || TokenRef::Shared(Arc::new(cfw::Cfw::default())), []),
        ("clw", 0x19, || TokenRef::Shared(Arc::new(clw::Clw::default())), []),
        (
            "sslt",
            0x1a,
            || TokenRef::Shared(Arc::new(sslt::Sslt::default())),
            [SyntaxDef::req(SyntaxToken::String), SyntaxDef::req(SyntaxToken::Usize)],
        ),
        (
            "ctc",
            0x1b,
            || TokenRef::Shared(Arc::new(ctc::Ctc::default())),
            [SyntaxDef::req(SyntaxToken::Usize), SyntaxDef::req(SyntaxToken::Usize)],
        ),
        (
            "ctr",
            0x1c,
            || TokenRef::Shared(Arc::new(ctr::Ctr::default())),
            [SyntaxDef::req(SyntaxToken::Usize), SyntaxDef::req(SyntaxToken::Usize)],
        ),
        (
            "cts",
            0x1d,
            || TokenRef::Shared(Arc::new(cts::Cts::default())),
            [SyntaxDef::req(SyntaxToken::Usize)],
        ),
        (
            "rlw",
            0x1e,
            || TokenRef::Shared(Arc::new(rlw::Rlw::default())),
            [SyntaxDef::req(SyntaxToken::String), SyntaxDef::req(SyntaxToken::String)],
        ),
        (
            "rnw",
            0x1f,
            || TokenRef::Shared(Arc::new(rnw::Rnw::default())),
            [
                SyntaxDef::req(SyntaxToken::String),
                SyntaxDef::req(SyntaxToken::String),
                SyntaxDef::req(SyntaxToken::Usize),
            ],
        ),
        ("urle", 0x20, || TokenRef::Shared(Arc::new(urle::Urle::default())), []),
        ("urld", 0x21, || TokenRef::Shared(Arc::new(urld::Urld::default())), []),
        ("rev", 0x22, || TokenRef::Shared(Arc::new(rev::Rev::default())), []),
        ("splc", 0x23, || TokenRef::Shared(Arc::new(splc::Splc::default())), []),
        ("htmle", 0x24, || TokenRef::Shared(Arc::new(htmle::Htmle::default())), []),
        ("htmlu", 0x25, || TokenRef::Shared(Arc::new(htmlu::Htmlu::default())), []),
        ("jsone", 0x26, || TokenRef::Shared(Arc::new(jsone::Jsone::default())), []),
        ("jsonu", 0x27, || TokenRef::Shared(Arc::new(jsonu::Jsonu::default())), []),
        (
            "ins",
            0x28,
            || TokenRef::Shared(Arc::new(ins::Ins::default())),
            [SyntaxDef::req(SyntaxToken::Usize), SyntaxDef::req(SyntaxToken::String)],
        ),
        (
            "tlcw",
            0x29,
            || TokenRef::Shared(Arc::new(tlcw::Tlcw::default())),
            [SyntaxDef::req(SyntaxToken::Usize)],
        ),
        (
            "tucw",
            0x2a,
            || TokenRef::Shared(Arc::new(tucw::Tucw::default())),
            [SyntaxDef::req(SyntaxToken::Usize)],
        ),
        ("jkbc", 0x2b, || TokenRef::Shared(Arc::new(jkbc::Jkbc::default())), []),
        ("jsnc", 0x2c, || TokenRef::Shared(Arc::new(jsnc::Jsnc::default())), []),
        ("jcmc", 0x2d, || TokenRef::Shared(Arc::new(jcmc::Jcmc::default())), []),
        ("jpsc", 0x2e, || TokenRef::Shared(Arc::new(jpsc::Jpsc::default())), []),
        (
            "padl",
            0x2f,
            || TokenRef::Shared(Arc::new(padl::Padl::default())),
            [SyntaxDef::req(SyntaxToken::String), SyntaxDef::req(SyntaxToken::Usize)],
        ),
        (
            "padr",
            0x30,
            || TokenRef::Shared(Arc::new(padr::Padr::default())),
            [SyntaxDef::req(SyntaxToken::String), SyntaxDef::req(SyntaxToken::Usize)],
        ),
        ("rmws", 0x31, || TokenRef::Shared(Arc::new(rmws::Rmws::default())), []),
        (
            "dls",
            0x32,
            || TokenRef::Shared(Arc::new(dls::Dls::default())),
            [SyntaxDef::req(SyntaxToken::Usize)],
        ),
        (
            "ifdc",
            0x33,
            || TokenRef::Shared(Arc::new(ifdc::Ifdc::default())),
            [
                SyntaxDef::req(SyntaxToken::String),
                SyntaxDef::req(SyntaxToken::Literal("do")),
                SyntaxDef::req(SyntaxToken::Token),
            ],
        ),
        (
            "blk",
            0x34,
            || TokenRef::Shared(Arc::new(blk::Blk::default())),
            [
                SyntaxDef::req(SyntaxToken::String),
                SyntaxDef::req(SyntaxToken::Literal("assoc")),
                SyntaxDef::req(SyntaxToken::Token),
            ],
        ),
        (
            "cblk",
            0x35,
            || TokenRef::Shared(Arc::new(cblk::Cblk::default())),
            [SyntaxDef::req(SyntaxToken::String)],
        ),
        (
            "val",
            0x36,
            || TokenRef::Shared(Arc::new(val::Val::default())),
            [
                SyntaxDef::req(SyntaxToken::String),
                SyntaxDef::req(SyntaxToken::Literal("=")),
                SyntaxDef::req(SyntaxToken::String),
            ],
        ),
        (
            "emj",
            0x37,
            || TokenRef::Shared(Arc::new(emj::Emj::default())),
            [SyntaxDef::req(SyntaxToken::String), SyntaxDef::req(SyntaxToken::String)],
        ),
        ("null", 0x38, || TokenRef::Shared(Arc::new(null::Null::default())), []),
        (
            "var",
            0x39,
            || TokenRef::Shared(Arc::new(var::Var::default())),
            [
                SyntaxDef::req(SyntaxToken::String),
                SyntaxDef::req(SyntaxToken::Literal("=")),
                SyntaxDef::req(SyntaxToken::String),
            ],
        ),
        (
            "mutv",
            0x3a,
            || TokenRef::Shared(Arc::new(mutv::Mutv::default())),
            [
                SyntaxDef::req(SyntaxToken::String),
                SyntaxDef::req(SyntaxToken::Literal("=")),
                SyntaxDef::req(SyntaxToken::String),
            ],
        ),
        (
            "eval",
            0x3b,
            || TokenRef::Shared(Arc::new(eval::Eval::default())),
            [
                SyntaxDef::req(SyntaxToken::String),
                SyntaxDef::req(SyntaxToken::Literal("in")),
                SyntaxDef::req(SyntaxToken::String),
            ],
        ),
        (
            "iter",
            0x3c,
            || TokenRef::Shared(Arc::new(iter::Iter::default())),
            [
                SyntaxDef::req(SyntaxToken::Usize),
                SyntaxDef::req(SyntaxToken::Literal("times")),
                SyntaxDef::req(SyntaxToken::Token),
            ],
        ),
        (
            "rmp",
            0x3d,
            || TokenRef::Shared(Arc::new(rmp::Rmp::default())),
            [SyntaxDef::req(SyntaxToken::String)],
        ),
        ("rmapd", 0x3f, || TokenRef::Shared(Arc::new(rmapd::Rmapd::default())), [
            SyntaxDef::req(SyntaxToken::String), SyntaxDef::req(SyntaxToken::Literal("add")),
            SyntaxDef::req(SyntaxToken::String), SyntaxDef::req(SyntaxToken::String),
        ]),
        ("rmape", 0x40, || TokenRef::Shared(Arc::new(rmape::Rmape::default())), [SyntaxDef::req(SyntaxToken::String)]),
        ("unorm", 0x41, || TokenRef::Shared(Arc::new(unorm::Unorm::default())), [SyntaxDef::req(SyntaxToken::String)]),
        ("end", 0x3e, || TokenRef::Shared(Arc::new(end::End::default())), []),
        ("apl", 0x42, || TokenRef::Shared(Arc::new(apl::Apl::default())), [SyntaxDef::req(SyntaxToken::String)]),
        ("dln", 0x43, || TokenRef::Shared(Arc::new(dln::Dln::default())), [SyntaxDef::req(SyntaxToken::Usize)]),
        ("dlna", 0x44, || TokenRef::Shared(Arc::new(dlna::Dlna::default())), [SyntaxDef::req(SyntaxToken::Usize)]),
        ("dlnb", 0x45, || TokenRef::Shared(Arc::new(dlnb::Dlnb::default())), [SyntaxDef::req(SyntaxToken::Usize)]),
        ("slnc", 0x46, || TokenRef::Shared(Arc::new(slnc::Slnc::default())), [SyntaxDef::req(SyntaxToken::Usize), SyntaxDef::req(SyntaxToken::Usize)]),
        ("dlni", 0x47, || TokenRef::Shared(Arc::new(dlni::Dlni::default())), [SyntaxDef::req(SyntaxToken::Usize), SyntaxDef::req(SyntaxToken::Usize)]),
        ("inl", 0x48, || TokenRef::Shared(Arc::new(inl::Inl::default())), [SyntaxDef::req(SyntaxToken::Usize), SyntaxDef::req(SyntaxToken::String)]),
        ("nln", 0x49, || TokenRef::Shared(Arc::new(nln::Nln::default())), []),
        ("rln", 0x4a, || TokenRef::Shared(Arc::new(rln::Rln::default())), [SyntaxDef::req(SyntaxToken::Usize), SyntaxDef::req(SyntaxToken::String)]),
        ("pln", 0x4b, || TokenRef::Shared(Arc::new(pln::Pln::default())), [SyntaxDef::req(SyntaxToken::String)]),
        ("dle", 0x4c, || TokenRef::Shared(Arc::new(dle::Dle::default())), []),
        ("klm", 0x4d, || TokenRef::Shared(Arc::new(klm::Klm::default())), [SyntaxDef::req(SyntaxToken::String)]),
        ("dlm", 0x4e, || TokenRef::Shared(Arc::new(dlm::Dlm::default())), [SyntaxDef::req(SyntaxToken::String)]),
        ("revln", 0x4f, || TokenRef::Shared(Arc::new(revln::Revln::default())), []),
        ("uniqln", 0x50, || TokenRef::Shared(Arc::new(uniqln::Uniqln::default())), []),
        ("joinln", 0x51, || TokenRef::Shared(Arc::new(joinln::Joinln::default())), [SyntaxDef::req(SyntaxToken::String)]),
    ];
}
