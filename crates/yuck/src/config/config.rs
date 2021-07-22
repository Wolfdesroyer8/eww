use std::collections::HashMap;

use codespan_reporting::files::SimpleFiles;
use simplexpr::SimplExpr;

use super::{
    file_provider::{FilesError, YuckFiles},
    script_var_definition::ScriptVarDefinition,
    var_definition::VarDefinition,
    widget_definition::WidgetDefinition,
    widget_use::WidgetUse,
    window_definition::WindowDefinition,
};
use crate::{
    config::script_var_definition::{PollScriptVar, TailScriptVar},
    error::{AstError, AstResult, OptionAstErrorExt},
    parser::{
        ast::Ast,
        ast_iterator::AstIterator,
        from_ast::{FromAst, FromAstElementContent},
    },
};
use eww_shared_util::{AttrName, Span, VarName};

#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize)]
pub struct Include {
    pub path: String,
    pub path_span: Span,
}

impl FromAstElementContent for Include {
    fn get_element_name() -> &'static str {
        "include"
    }

    fn from_tail<I: Iterator<Item = Ast>>(span: Span, mut iter: AstIterator<I>) -> AstResult<Self> {
        let (path_span, path) = iter.expect_literal()?;
        Ok(Include { path: path.to_string(), path_span })
    }
}

pub enum TopLevel {
    Include(Include),
    VarDefinition(VarDefinition),
    ScriptVarDefinition(ScriptVarDefinition),
    WidgetDefinition(WidgetDefinition),
    WindowDefinition(WindowDefinition),
}

impl FromAst for TopLevel {
    fn from_ast(e: Ast) -> AstResult<Self> {
        let span = e.span();
        let mut iter = e.try_ast_iter()?;
        let (sym_span, element_name) = iter.expect_symbol()?;
        Ok(match element_name.as_str() {
            x if x == Include::get_element_name() => Self::Include(Include::from_tail(span, iter)?),
            x if x == WidgetDefinition::get_element_name() => Self::WidgetDefinition(WidgetDefinition::from_tail(span, iter)?),
            x if x == VarDefinition::get_element_name() => Self::VarDefinition(VarDefinition::from_tail(span, iter)?),
            x if x == PollScriptVar::get_element_name() => {
                Self::ScriptVarDefinition(ScriptVarDefinition::Poll(PollScriptVar::from_tail(span, iter)?))
            }
            x if x == TailScriptVar::get_element_name() => {
                Self::ScriptVarDefinition(ScriptVarDefinition::Tail(TailScriptVar::from_tail(span, iter)?))
            }
            x if x == WindowDefinition::get_element_name() => Self::WindowDefinition(WindowDefinition::from_tail(span, iter)?),
            x => return Err(AstError::UnknownToplevel(sym_span, x.to_string())),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize)]
pub struct Config {
    pub widget_definitions: HashMap<String, WidgetDefinition>,
    pub window_definitions: HashMap<String, WindowDefinition>,
    pub var_definitions: HashMap<VarName, VarDefinition>,
    pub script_vars: HashMap<VarName, ScriptVarDefinition>,
}

impl Config {
    fn append_toplevel(&mut self, files: &mut impl YuckFiles, toplevel: TopLevel) -> AstResult<()> {
        match toplevel {
            TopLevel::VarDefinition(x) => {
                self.var_definitions.insert(x.name.clone(), x);
            }
            TopLevel::ScriptVarDefinition(x) => {
                self.script_vars.insert(x.name().clone(), x);
            }
            TopLevel::WidgetDefinition(x) => {
                self.widget_definitions.insert(x.name.clone(), x);
            }
            TopLevel::WindowDefinition(x) => {
                self.window_definitions.insert(x.name.clone(), x);
            }
            TopLevel::Include(include) => {
                let (file_id, toplevels) = files.load(&include.path).map_err(|err| match err {
                    FilesError::IoError(_) => AstError::IncludedFileNotFound(include),
                    FilesError::AstError(x) => x,
                })?;
                for element in toplevels {
                    self.append_toplevel(files, TopLevel::from_ast(element)?)?;
                }
            }
        }
        Ok(())
    }

    pub fn generate(files: &mut impl YuckFiles, elements: Vec<Ast>) -> AstResult<Self> {
        let mut config = Self {
            widget_definitions: HashMap::new(),
            window_definitions: HashMap::new(),
            var_definitions: HashMap::new(),
            script_vars: HashMap::new(),
        };
        for element in elements {
            config.append_toplevel(files, TopLevel::from_ast(element)?)?;
        }
        Ok(config)
    }

    pub fn generate_from_main_file(files: &mut impl YuckFiles, path: &str) -> AstResult<Self> {
        let (span, top_levels) = files.load(path).map_err(|err| AstError::Other(None, Box::new(err)))?;
        Self::generate(files, top_levels)
    }
}