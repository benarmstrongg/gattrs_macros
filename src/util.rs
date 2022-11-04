use std::collections::HashMap;

use syn::parse::ParseStream;
use syn::{Expr, ExprArray, ExprAssign, ExprLit, ExprPath, Lit, Path, Result};

pub enum ArgValueType {
    Str(Option<ExprLit>),
    Bool(Option<ExprLit>),
    VecStr(Option<ExprArray>),
}

fn get_expression_name_and_val(
    expr: &Expr,
    stream: &ParseStream,
) -> Result<(String, ArgValueType)> {
    match &*expr {
        Expr::Assign(ExprAssign { left, right, .. }) => match left.as_ref() {
            Expr::Path(ExprPath {
                path: Path { segments, .. },
                ..
            }) => {
                let name = segments.first().unwrap().ident.to_string();
                let value = match right.as_ref() {
                    // Expr::Lit(literal) => Ok(ArgValueType::Str(Some(literal.to_owned()))),
                    Expr::Lit(literal) => match literal {
                        ExprLit {
                            lit: Lit::Bool(_), ..
                        } => Ok(ArgValueType::Bool(Some(literal.to_owned()))),
                        ExprLit {
                            lit: Lit::Str(_), ..
                        } => Ok(ArgValueType::Str(Some(literal.to_owned()))),
                        _ => Err(stream.error(
                            "Right side of expression must be string literal or boolean value",
                        )),
                    },
                    Expr::Array(arr) => Ok(ArgValueType::VecStr(Some(arr.to_owned()))),
                    _ => Err(stream
                        .error("Right side of expression must be string literal or boolean value")),
                };
                match value {
                    Ok(val) => Ok((name, val)),
                    Err(err) => Err(err),
                }
            }
            _ => Err(stream.error("Args must be comma separated")),
        },
        _ => Err(stream.error("Arguments must be expressions (ex: primary = false)")),
    }
}

pub fn get_valid_arg(
    arg_map: &HashMap<String, ArgValueType>,
    expr: &Expr,
    stream: &ParseStream,
) -> Result<(String, ArgValueType)> {
    let (arg_name, arg_value) = get_expression_name_and_val(&expr, &stream)?;

    if !arg_map.contains_key(&arg_name) {
        return Err(stream.error(format!("Unknown arg {arg_name}")));
    }

    let arg_type = arg_map.get(&arg_name).unwrap().to_owned();

    let value = match arg_type {
        ArgValueType::Str(_) => match arg_value {
            ArgValueType::Str(value) => Ok(ArgValueType::Str(value)),
            _ => Err(stream.error(format!("Arg {arg_name} must be of type String"))),
        },
        ArgValueType::Bool(_) => match arg_value {
            ArgValueType::Bool(value) => Ok(ArgValueType::Bool(value)),
            _ => Err(stream.error(format!("Arg {arg_name} must be of type bool"))),
        },
        ArgValueType::VecStr(_) => match arg_value {
            ArgValueType::VecStr(value) => Ok(ArgValueType::VecStr(value)),
            _ => Err(stream.error(format!("Arg {arg_name} must be of type Vec<String>"))),
        },
    };

    match value {
        Ok(value) => Ok((arg_name, value)),
        Err(err) => Err(err),
    }
}
