//! Cover Grammar for Destructuring Assignment

use oxc_allocator::Box;
use oxc_ast::{ast::*, GetSpan};
use oxc_diagnostics::Result;

use crate::{diagnostics, Parser};

pub trait CoverGrammar<'a, T>: Sized {
    fn cover(value: T, p: &mut Parser<'a>) -> Result<Self>;
}

impl<'a> CoverGrammar<'a, Expression<'a>> for AssignmentTarget<'a> {
    fn cover(expr: Expression<'a>, p: &mut Parser<'a>) -> Result<Self> {
        match expr {
            Expression::ArrayExpression(array_expr) => {
                ArrayAssignmentTarget::cover(Box::into_inner(array_expr), p)
                    .map(|pat| p.ast.alloc(pat))
                    .map(AssignmentTargetPattern::ArrayAssignmentTarget)
                    .map(AssignmentTarget::AssignmentTargetPattern)
            }
            Expression::ObjectExpression(object_expr) => {
                ObjectAssignmentTarget::cover(Box::into_inner(object_expr), p)
                    .map(|pat| p.ast.alloc(pat))
                    .map(AssignmentTargetPattern::ObjectAssignmentTarget)
                    .map(AssignmentTarget::AssignmentTargetPattern)
            }
            _ => {
                SimpleAssignmentTarget::cover(expr, p).map(AssignmentTarget::SimpleAssignmentTarget)
            }
        }
    }
}

impl<'a> CoverGrammar<'a, Expression<'a>> for SimpleAssignmentTarget<'a> {
    #[allow(clippy::only_used_in_recursion)]
    fn cover(expr: Expression<'a>, p: &mut Parser<'a>) -> Result<Self> {
        match expr {
            Expression::Identifier(ident) => {
                Ok(SimpleAssignmentTarget::AssignmentTargetIdentifier(ident))
            }
            Expression::MemberExpression(expr) => {
                Ok(SimpleAssignmentTarget::MemberAssignmentTarget(expr))
            }
            Expression::ParenthesizedExpression(expr) => {
                let span = expr.span;
                match Box::into_inner(expr).expression {
                    Expression::ObjectExpression(_) | Expression::ArrayExpression(_) => {
                        Err(diagnostics::InvalidAssignment(span).into())
                    }
                    expr => SimpleAssignmentTarget::cover(expr, p),
                }
            }
            Expression::TSAsExpression(expr) => Ok(SimpleAssignmentTarget::TSAsExpression(expr)),
            Expression::TSNonNullExpression(expr) => {
                Ok(SimpleAssignmentTarget::TSNonNullExpression(expr))
            }
            Expression::TSTypeAssertion(expr) => Ok(SimpleAssignmentTarget::TSTypeAssertion(expr)),
            expr => Err(diagnostics::InvalidAssignment(expr.span()).into()),
        }
    }
}

impl<'a> CoverGrammar<'a, ArrayExpression<'a>> for ArrayAssignmentTarget<'a> {
    fn cover(expr: ArrayExpression<'a>, p: &mut Parser<'a>) -> Result<Self> {
        let mut elements = p.ast.new_vec();
        let mut rest = None;

        let len = expr.elements.len();
        for (i, elem) in expr.elements.into_iter().enumerate() {
            if let Some(argument) = elem {
                match argument {
                    Argument::Expression(expr) => {
                        let target = AssignmentTargetMaybeDefault::cover(expr, p)?;
                        elements.push(Some(target));
                    }
                    Argument::SpreadElement(elem) => {
                        if i == len - 1 {
                            rest =
                                Some(AssignmentTarget::cover(Box::into_inner(elem).argument, p)?);
                            if let Some(span) = expr.trailing_comma {
                                p.error(diagnostics::RestElementTraillingComma(span));
                            }
                        } else {
                            return Err(diagnostics::SpreadLastElement(elem.span).into());
                        }
                    }
                }
            } else {
                elements.push(None);
            }
        }

        Ok(ArrayAssignmentTarget {
            span: expr.span,
            elements,
            rest,
            trailing_comma: expr.trailing_comma,
        })
    }
}

impl<'a> CoverGrammar<'a, Expression<'a>> for AssignmentTargetMaybeDefault<'a> {
    fn cover(expr: Expression<'a>, p: &mut Parser<'a>) -> Result<Self> {
        match expr {
            Expression::AssignmentExpression(assignment_expr) => {
                let target =
                    AssignmentTargetWithDefault::cover(Box::into_inner(assignment_expr), p)?;
                Ok(AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(p.ast.alloc(target)))
            }
            expr => {
                let target = AssignmentTarget::cover(expr, p)?;
                Ok(AssignmentTargetMaybeDefault::AssignmentTarget(p.ast.alloc(target)))
            }
        }
    }
}

impl<'a> CoverGrammar<'a, AssignmentExpression<'a>> for AssignmentTargetWithDefault<'a> {
    fn cover(expr: AssignmentExpression<'a>, _p: &mut Parser<'a>) -> Result<Self> {
        Ok(Self { span: expr.span, binding: expr.left, init: expr.right })
    }
}

impl<'a> CoverGrammar<'a, ObjectExpression<'a>> for ObjectAssignmentTarget<'a> {
    fn cover(expr: ObjectExpression<'a>, p: &mut Parser<'a>) -> Result<Self> {
        let mut properties = p.ast.new_vec();
        let mut rest = None;

        let len = expr.properties.len();
        for (i, elem) in expr.properties.into_iter().enumerate() {
            match elem {
                ObjectProperty::Property(property) => {
                    let target = AssignmentTargetProperty::cover(Box::into_inner(property), p)?;
                    properties.push(target);
                }
                ObjectProperty::SpreadProperty(spread) => {
                    if i == len - 1 {
                        rest = Some(AssignmentTarget::cover(Box::into_inner(spread).argument, p)?);
                    } else {
                        return Err(diagnostics::SpreadLastElement(spread.span).into());
                    }
                }
            }
        }

        Ok(Self { span: expr.span, properties, rest })
    }
}

impl<'a> CoverGrammar<'a, Property<'a>> for AssignmentTargetProperty<'a> {
    fn cover(property: Property<'a>, p: &mut Parser<'a>) -> Result<Self> {
        if property.shorthand {
            let binding = match property.key {
                PropertyKey::Identifier(ident) => {
                    IdentifierReference { span: ident.span, name: Box::into_inner(ident).name }
                }
                _ => return p.unexpected(),
            };
            let init = match property.value {
                PropertyValue::Expression(Expression::AssignmentExpression(assignment_expr)) => {
                    Some(Box::into_inner(assignment_expr).right)
                }
                _ => None,
            };
            let target = AssignmentTargetPropertyIdentifier { span: property.span, binding, init };
            Ok(AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(p.ast.alloc(target)))
        } else {
            let binding = match property.value {
                PropertyValue::Expression(expr) => AssignmentTargetMaybeDefault::cover(expr, p)?,
                PropertyValue::Pattern(_) => {
                    return Err(diagnostics::InvalidAssignment(property.value.span()).into());
                }
            };
            let target = AssignmentTargetPropertyProperty {
                span: property.span,
                name: property.key,
                binding,
            };
            Ok(AssignmentTargetProperty::AssignmentTargetPropertyProperty(p.ast.alloc(target)))
        }
    }
}
