use anyhow::Result;
use nom::combinator::cond;

use crate::ast::{ForLoop, parse_code_block, parse_expression, parse_statement, parse_variable_declaration, Statement, WhileLoop};
use crate::lexer::LELexer;

pub fn parse_while_loop(lexer: &mut LELexer) -> Result<WhileLoop> {
    lexer.consume_keyword()?;
    lexer.consume_left_par()?;
    let cond = if let Ok(expr) = parse_expression(lexer) {
        Some(expr)
    } else {
        None
    };
    lexer.consume_right_par()?;
    let code_block = parse_code_block(lexer)?;
    Ok(WhileLoop {
        condition: cond,
        code_block,
    })
}