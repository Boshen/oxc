use std::fmt::Display;

use num_bigint::BigUint;
use oxc_allocator::{Box, Vec};
use serde::Serialize;

#[allow(clippy::wildcard_imports)]
use crate::{ast::*, Atom, SourceType, Span};

#[derive(Debug, PartialEq, Hash)]
pub struct Program<'a> {
    pub span: Span,
    pub directives: Vec<'a, Directive<'a>>,
    pub body: Vec<'a, Statement<'a>>,
    pub source_type: SourceType,
}

// SAFETY: The AST is part of the bump allocator,
// it is our responsibility to never simultaneously mutate across threads.
#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl<'a> Send for Program<'a> {}
unsafe impl<'a> Sync for Program<'a> {}

impl<'a> Program<'a> {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.body.is_empty() && self.directives.is_empty()
    }
}

/// Section 13 Expression
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum Expression<'a> {
    BooleanLiteral(Box<'a, BooleanLiteral>),
    NullLiteral(Box<'a, NullLiteral>),
    NumberLiteral(Box<'a, NumberLiteral<'a>>),
    BigintLiteral(Box<'a, BigintLiteral>),
    RegExpLiteral(Box<'a, RegExpLiteral>),
    StringLiteral(Box<'a, StringLiteral>),
    TemplateLiteral(Box<'a, TemplateLiteral<'a>>),

    Identifier(Box<'a, IdentifierReference>),

    MetaProperty(Box<'a, MetaProperty>),
    Super(Box<'a, Super>),

    ArrayExpression(Box<'a, ArrayExpression<'a>>),
    ArrowFunctionExpression(Box<'a, ArrowExpression<'a>>),
    AssignmentExpression(Box<'a, AssignmentExpression<'a>>),
    AwaitExpression(Box<'a, AwaitExpression<'a>>),
    BinaryExpression(Box<'a, BinaryExpression<'a>>),
    CallExpression(Box<'a, CallExpression<'a>>),
    ChainExpression(Box<'a, ChainExpression<'a>>),
    ClassExpression(Box<'a, Class<'a>>),
    ConditionalExpression(Box<'a, ConditionalExpression<'a>>),
    FunctionExpression(Box<'a, Function<'a>>),
    ImportExpression(Box<'a, ImportExpression<'a>>),
    LogicalExpression(Box<'a, LogicalExpression<'a>>),
    MemberExpression(Box<'a, MemberExpression<'a>>),
    NewExpression(Box<'a, NewExpression<'a>>),
    ObjectExpression(Box<'a, ObjectExpression<'a>>),
    ParenthesizedExpression(Box<'a, ParenthesizedExpression<'a>>),
    SequenceExpression(Box<'a, SequenceExpression<'a>>),
    TaggedTemplateExpression(Box<'a, TaggedTemplateExpression<'a>>),
    ThisExpression(Box<'a, ThisExpression>),
    UnaryExpression(Box<'a, UnaryExpression<'a>>),
    UpdateExpression(Box<'a, UpdateExpression<'a>>),
    YieldExpression(Box<'a, YieldExpression<'a>>),
    PrivateInExpression(Box<'a, PrivateInExpression<'a>>),

    JSXElement(Box<'a, JSXElement<'a>>),
    JSXFragment(Box<'a, JSXFragment<'a>>),

    TSAsExpression(Box<'a, TSAsExpression<'a>>),
    TSTypeAssertion(Box<'a, TSTypeAssertion<'a>>),
    TSNonNullExpression(Box<'a, TSNonNullExpression<'a>>),
    TSInstantiationExpression(Box<'a, TSInstantiationExpression<'a>>),
}

impl<'a> Expression<'a> {
    /// `PrimaryExpression`
    /// [tc39/ecma262#prod-PrimaryExpression](https://tc39.es/ecma262/#prod-PrimaryExpression)
    #[must_use]
    pub fn is_primary_expression(&self) -> bool {
        self.is_literal_expression()
            || matches!(
                self,
                Self::Identifier(_)
                    | Self::ThisExpression(_)
                    | Self::FunctionExpression(_)
                    | Self::ClassExpression(_)
                    | Self::ParenthesizedExpression(_)
                    | Self::ArrayExpression(_)
                    | Self::ObjectExpression(_)
            )
    }

    #[must_use]
    pub fn is_literal_expression(&self) -> bool {
        matches!(
            self,
            Self::BooleanLiteral(_)
                | Self::NullLiteral(_)
                | Self::NumberLiteral(_)
                | Self::BigintLiteral(_)
                | Self::RegExpLiteral(_)
                | Self::StringLiteral(_) // TemplateLiteral is not `Literal` type per oxc_ast
        )
    }

    #[must_use]
    pub fn is_string_literal(&self) -> bool {
        matches!(self, Self::StringLiteral(_) | Self::TemplateLiteral(_))
    }

    /// Determines whether the given expr is a `null` literal
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self, Expression::NullLiteral(_))
    }

    /// Determines whether the given expr is a `undefined` literal
    #[must_use]
    pub fn is_undefined(&self) -> bool {
        matches!(self, Self::Identifier(ident) if ident.name == "undefined")
    }

    /// Determines whether the given expr is a `void 0`
    #[must_use]
    pub fn is_void_0(&self) -> bool {
        matches!(self, Self::UnaryExpression(expr) if expr.operator == UnaryOperator::Void)
    }

    /// Determines whether the given expr evaluate to `undefined`
    #[must_use]
    pub fn evaluate_to_undefined(&self) -> bool {
        self.is_undefined() || self.is_void_0()
    }

    /// Determines whether the given expr is a `null` or `undefined` or `void 0`
    #[must_use]
    pub fn is_null_or_undefined(&self) -> bool {
        self.is_null() || self.evaluate_to_undefined()
    }

    /// Remove nested parentheses from this expression.
    #[must_use]
    pub fn without_parenthesized(&self) -> &Self {
        match self {
            Expression::ParenthesizedExpression(expr) => expr.expression.without_parenthesized(),
            _ => self,
        }
    }

    #[must_use]
    pub fn is_specific_id(&self, name: &str) -> bool {
        match self.get_inner_expression() {
            Expression::Identifier(ident) => ident.name == name,
            _ => false,
        }
    }

    #[must_use]
    pub fn is_specific_member_access(&'a self, object: &str, property: &str) -> bool {
        match self.get_inner_expression() {
            Expression::MemberExpression(expr) => expr.is_specific_member_access(object, property),
            Expression::ChainExpression(chain) => {
                let ChainElement::MemberExpression(expr) = &chain.expression else {
                return false;
              };
                expr.is_specific_member_access(object, property)
            }
            _ => false,
        }
    }

    #[must_use]
    pub fn get_inner_expression(&self) -> &Expression<'a> {
        match self {
            Expression::ParenthesizedExpression(expr) => expr.expression.get_inner_expression(),
            Expression::TSAsExpression(expr) => expr.expression.get_inner_expression(),
            Expression::TSInstantiationExpression(expr) => expr.expression.get_inner_expression(),
            Expression::TSNonNullExpression(expr) => expr.expression.get_inner_expression(),
            Expression::TSTypeAssertion(expr) => expr.expression.get_inner_expression(),
            _ => self,
        }
    }

    #[must_use]
    pub fn get_identifier_reference(&self) -> Option<&IdentifierReference> {
        match self.get_inner_expression() {
            Expression::Identifier(ident) => Some(ident),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_function(&self) -> bool {
        matches!(self, Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_))
    }

    /// Returns literal's value converted to the Boolean type
    /// returns `true` when node is truthy, `false` when node is falsy, `None` when it cannot be determined.
    #[must_use]
    pub fn get_boolean_value(&self) -> Option<bool> {
        match self {
            Self::BooleanLiteral(lit) => Some(lit.value),
            Self::NullLiteral(_) => Some(false),
            Self::NumberLiteral(lit) => Some(lit.value != 0.0),
            Self::BigintLiteral(lit) => Some(lit.value != BigUint::new(vec![])),
            Self::RegExpLiteral(_) => Some(true),
            Self::StringLiteral(lit) => Some(!lit.value.is_empty()),
            _ => None,
        }
    }
}

/// Section 12.6 `IdentifierName`
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub struct IdentifierName {
    #[serde(flatten)]
    pub span: Span,
    pub name: Atom,
}

/// Section 13.1 `IdentifierReference`
#[derive(Debug, Clone, Serialize, PartialEq, Hash, Eq)]
#[serde(tag = "type")]
pub struct IdentifierReference {
    #[serde(flatten)]
    pub span: Span,
    pub name: Atom,
}

/// Section 13.1 `BindingIdentifier`
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub struct BindingIdentifier {
    #[serde(flatten)]
    pub span: Span,
    pub name: Atom,
}

/// Section 13.1 `LabelIdentifier`
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub struct LabelIdentifier {
    #[serde(flatten)]
    pub span: Span,
    pub name: Atom,
}

/// Section 13.2.2 This Expression
#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub struct ThisExpression {
    #[serde(flatten)]
    pub span: Span,
}

/// Section 13.2.5 Array Expression
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ArrayExpression<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub elements: Vec<'a, Option<Argument<'a>>>,
    pub trailing_comma: Option<Span>,
}

/// Section 13.2.6 Object Expression
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ObjectExpression<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub properties: Vec<'a, ObjectProperty<'a>>,
    pub trailing_comma: Option<Span>,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum ObjectProperty<'a> {
    Property(Box<'a, Property<'a>>),
    SpreadProperty(Box<'a, SpreadElement<'a>>),
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct Property<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub kind: PropertyKind,
    pub key: PropertyKey<'a>,
    pub value: PropertyValue<'a>,
    pub method: bool,
    pub shorthand: bool,
    pub computed: bool,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum PropertyKey<'a> {
    Identifier(Box<'a, IdentifierName>),
    PrivateIdentifier(Box<'a, PrivateIdentifier>),
    Expression(Expression<'a>),
}

impl<'a> PropertyKey<'a> {
    #[must_use]
    pub fn static_name(&self) -> Option<Atom> {
        match self {
            Self::Identifier(ident) => Some(ident.name.clone()),
            Self::PrivateIdentifier(_) => None,
            Self::Expression(expr) => match expr {
                Expression::StringLiteral(lit) => Some(lit.value.clone()),
                Expression::RegExpLiteral(lit) => Some(Atom::from(lit.regex.to_string())),
                Expression::NumberLiteral(lit) => Some(Atom::from(lit.value.to_string())),
                Expression::BigintLiteral(lit) => Some(Atom::from(lit.value.to_string())),
                Expression::NullLiteral(_) => Some("null".into()),
                Expression::TemplateLiteral(lit) => {
                    lit.expressions.is_empty().then(|| lit.quasi()).flatten().cloned()
                }
                _ => None,
            },
        }
    }

    #[must_use]
    pub fn is_private_identifier(&self) -> bool {
        matches!(self, Self::PrivateIdentifier(_))
    }
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum PropertyValue<'a> {
    // for AssignmentProperty in ObjectPattern https://github.com/oxc_ast/oxc_ast/blob/master/es2015.md#objectpattern
    Pattern(BindingPattern<'a>),
    Expression(Expression<'a>),
}

#[derive(Debug, Clone, Copy, Serialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum PropertyKind {
    Init,
    Get,
    Set,
}

/// Section 13.2.9 Template Literal
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct TemplateLiteral<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub quasis: Vec<'a, TemplateElement>,
    pub expressions: Vec<'a, Expression<'a>>,
}

impl<'a> TemplateLiteral<'a> {
    #[must_use]
    pub fn is_no_substitution_template(&self) -> bool {
        self.expressions.is_empty() && self.quasis.len() == 1
    }

    /// Get single quasi from `template`
    #[must_use]
    pub fn quasi(&self) -> Option<&Atom> {
        self.quasis.first().and_then(|quasi| quasi.value.cooked.as_ref())
    }
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct TaggedTemplateExpression<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub tag: Expression<'a>,
    pub quasi: TemplateLiteral<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<Box<'a, TSTypeParameterInstantiation<'a>>>,
}

#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub struct TemplateElement {
    #[serde(flatten)]
    pub span: Span,
    pub tail: bool,
    pub value: TemplateElementValue,
}

#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
pub struct TemplateElementValue {
    pub raw: Atom,
    pub cooked: Option<Atom>,
}

/// Section 13.3 Member Expression
#[derive(Debug, PartialEq, Hash)]
pub enum MemberExpression<'a> {
    ComputedMemberExpression(ComputedMemberExpression<'a>),
    StaticMemberExpression(StaticMemberExpression<'a>),
    PrivateFieldExpression(PrivateFieldExpression<'a>),
}

impl<'a> MemberExpression<'a> {
    #[must_use]
    pub fn optional(&self) -> bool {
        match self {
            MemberExpression::ComputedMemberExpression(expr) => expr.optional,
            MemberExpression::StaticMemberExpression(expr) => expr.optional,
            MemberExpression::PrivateFieldExpression(expr) => expr.optional,
        }
    }

    #[must_use]
    pub fn object(&self) -> &Expression<'a> {
        match self {
            MemberExpression::ComputedMemberExpression(expr) => &expr.object,
            MemberExpression::StaticMemberExpression(expr) => &expr.object,
            MemberExpression::PrivateFieldExpression(expr) => &expr.object,
        }
    }

    #[must_use]
    pub fn static_property_name(&'a self) -> Option<&'a str> {
        match self {
            MemberExpression::ComputedMemberExpression(expr) => match &expr.expression {
                Expression::StringLiteral(lit) => Some(&lit.value),
                Expression::TemplateLiteral(lit) => {
                    if lit.expressions.is_empty() && lit.quasis.len() == 1 {
                        Some(&lit.quasis[0].value.raw)
                    } else {
                        None
                    }
                }
                _ => None,
            },
            MemberExpression::StaticMemberExpression(expr) => Some(&expr.property.name),
            MemberExpression::PrivateFieldExpression(_) => None,
        }
    }

    /// Whether it is a static member access `object.property`
    #[must_use]
    pub fn is_specific_member_access(&'a self, object: &str, property: &str) -> bool {
        self.object().is_specific_id(object)
            && self.static_property_name().is_some_and(|p| p == property)
    }
}

#[derive(Debug, PartialEq, Hash)]
pub struct ComputedMemberExpression<'a> {
    pub span: Span,
    pub object: Expression<'a>,
    pub expression: Expression<'a>,
    pub optional: bool, // for optional chaining
}

#[derive(Debug, PartialEq, Hash)]
pub struct StaticMemberExpression<'a> {
    pub span: Span,
    pub object: Expression<'a>,
    pub property: IdentifierName,
    pub optional: bool, // for optional chaining
}

#[derive(Debug, PartialEq, Hash)]
pub struct PrivateFieldExpression<'a> {
    pub span: Span,
    pub object: Expression<'a>,
    pub field: PrivateIdentifier,
    pub optional: bool, // for optional chaining
}

/// Section 13.3 Call Expression
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type", rename_all = "camelCase")]
pub struct CallExpression<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub callee: Expression<'a>,
    pub arguments: Vec<'a, Argument<'a>>,
    pub optional: bool, // for optional chaining
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<Box<'a, TSTypeParameterInstantiation<'a>>>,
}

impl<'a> CallExpression<'a> {
    #[must_use]
    pub fn is_require_call(&self) -> bool {
        if self.arguments.len() != 1 {
            return false;
        }
        if let Expression::Identifier(id) = &self.callee {
            id.name == "require"
                && matches!(
                    self.arguments.first(),
                    Some(Argument::Expression(
                        Expression::StringLiteral(_) | Expression::TemplateLiteral(_),
                    )),
                )
        } else {
            false
        }
    }

    #[must_use]
    pub fn is_symbol_or_symbol_for_call(&'a self) -> bool {
        // TODO: is 'Symbol' reference to global object
        match &self.callee {
            Expression::Identifier(id) => id.name == "Symbol",
            Expression::MemberExpression(member) => {
                matches!(member.object(), Expression::Identifier(id) if id.name == "Symbol")
                    && member.static_property_name() == Some("for")
            }
            _ => false,
        }
    }

    #[must_use]
    pub fn common_js_require(&self) -> Option<&StringLiteral> {
        if let Expression::Identifier(ident) = &self.callee
            && ident.name =="require"
            && self.arguments.len() == 1
            && let Argument::Expression(Expression::StringLiteral(str_literal)) = &self.arguments[0] {
            Some(str_literal)
        } else {
            None
        }
    }
}

/// Section 13.3 New Expression
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct NewExpression<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub callee: Expression<'a>,
    pub arguments: Vec<'a, Argument<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<Box<'a, TSTypeParameterInstantiation<'a>>>,
}

/// Section 13.3 Meta Property
/// `new.target` | `import.meta`
#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub struct MetaProperty {
    #[serde(flatten)]
    pub span: Span,
    pub meta: IdentifierName,
    pub property: IdentifierName,
}

/// Section 13.3 Spread Element
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct SpreadElement<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub argument: Expression<'a>,
}

/// Section 13.3 Argument
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum Argument<'a> {
    SpreadElement(Box<'a, SpreadElement<'a>>),
    Expression(Expression<'a>),
}

/// Section 13.4 Update Expression
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct UpdateExpression<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub operator: UpdateOperator,
    pub prefix: bool,
    pub argument: SimpleAssignmentTarget<'a>,
}

/// Section 13.5 Unary Expression
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct UnaryExpression<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub operator: UnaryOperator,
    pub prefix: bool,
    pub argument: Expression<'a>,
}

/// Section 13.6 - 13.13 Binary Expression
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct BinaryExpression<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub left: Expression<'a>,
    pub operator: BinaryOperator,
    pub right: Expression<'a>,
}

/// `RelationalExpression`[In, Yield, Await] :
///     [+In] `PrivateIdentifier` in `ShiftExpression`[?Yield, ?Await]
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct PrivateInExpression<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub left: PrivateIdentifier,
    pub operator: BinaryOperator, // BinaryOperator::In
    pub right: Expression<'a>,
}

/// Section 13.13 Binary Logical Operators
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct LogicalExpression<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub left: Expression<'a>,
    pub operator: LogicalOperator,
    pub right: Expression<'a>,
}

/// Section 13.14 Conditional Expression
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ConditionalExpression<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub test: Expression<'a>,
    pub consequent: Expression<'a>,
    pub alternate: Expression<'a>,
}

/// Section 13.15 Assignment Expression
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct AssignmentExpression<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub operator: AssignmentOperator,
    pub left: AssignmentTarget<'a>,
    pub right: Expression<'a>,
}

/// 13.15.5 Destructuring Assignment
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum AssignmentTarget<'a> {
    SimpleAssignmentTarget(SimpleAssignmentTarget<'a>),
    AssignmentTargetPattern(AssignmentTargetPattern<'a>),
}

impl<'a> AssignmentTarget<'a> {
    #[must_use]
    pub fn is_destructuring_pattern(&self) -> bool {
        matches!(self, Self::AssignmentTargetPattern(_))
    }
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum SimpleAssignmentTarget<'a> {
    AssignmentTargetIdentifier(Box<'a, IdentifierReference>),
    MemberAssignmentTarget(Box<'a, MemberExpression<'a>>),
    TSAsExpression(Box<'a, TSAsExpression<'a>>),
    TSNonNullExpression(Box<'a, TSNonNullExpression<'a>>),
    TSTypeAssertion(Box<'a, TSTypeAssertion<'a>>),
}

impl<'a> SimpleAssignmentTarget<'a> {
    #[must_use]
    pub fn get_expression(&self) -> Option<&Expression<'a>> {
        match self {
            Self::TSAsExpression(expr) => Some(&expr.expression),
            Self::TSNonNullExpression(expr) => Some(&expr.expression),
            Self::TSTypeAssertion(expr) => Some(&expr.expression),
            _ => None,
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum AssignmentTargetPattern<'a> {
    ArrayAssignmentTarget(Box<'a, ArrayAssignmentTarget<'a>>),
    ObjectAssignmentTarget(Box<'a, ObjectAssignmentTarget<'a>>),
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ArrayAssignmentTarget<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub elements: Vec<'a, Option<AssignmentTargetMaybeDefault<'a>>>,
    pub rest: Option<AssignmentTarget<'a>>,
    pub trailing_comma: Option<Span>,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ObjectAssignmentTarget<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub properties: Vec<'a, AssignmentTargetProperty<'a>>,
    pub rest: Option<AssignmentTarget<'a>>,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum AssignmentTargetMaybeDefault<'a> {
    AssignmentTarget(Box<'a, AssignmentTarget<'a>>),
    AssignmentTargetWithDefault(Box<'a, AssignmentTargetWithDefault<'a>>),
}

impl<'a> AssignmentTargetMaybeDefault<'a> {
    #[must_use]
    pub fn name(&self) -> Option<Atom> {
        let target = match self {
            Self::AssignmentTarget(target) => target,
            Self::AssignmentTargetWithDefault(target) => &target.binding,
        };

        if let AssignmentTarget::SimpleAssignmentTarget(
            SimpleAssignmentTarget::AssignmentTargetIdentifier(id),
        ) = target
        {
            Some(id.name.clone())
        } else {
            None
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct AssignmentTargetWithDefault<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub binding: AssignmentTarget<'a>,
    pub init: Expression<'a>,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum AssignmentTargetProperty<'a> {
    AssignmentTargetPropertyIdentifier(Box<'a, AssignmentTargetPropertyIdentifier<'a>>),
    AssignmentTargetPropertyProperty(Box<'a, AssignmentTargetPropertyProperty<'a>>),
}

/// `AssignmentProperty`[Yield, Await] :
///     `IdentifierReference`[?Yield, ?Await] Initializer[+In, ?Yield, ?Await]opt
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct AssignmentTargetPropertyIdentifier<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub binding: IdentifierReference,
    pub init: Option<Expression<'a>>,
}

/// `AssignmentProperty`[Yield, Await] :
///     `PropertyName`[?Yield, ?Await] : `AssignmentElement`[?Yield, ?Await]
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct AssignmentTargetPropertyProperty<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub name: PropertyKey<'a>,
    pub binding: AssignmentTargetMaybeDefault<'a>,
}

/// Section 13.16 Sequence Expression
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct SequenceExpression<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub expressions: Vec<'a, Expression<'a>>,
}

#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub struct Super {
    #[serde(flatten)]
    pub span: Span,
}

/// Section 15.8 Await Expression
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct AwaitExpression<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub argument: Expression<'a>,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ChainExpression<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub expression: ChainElement<'a>,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum ChainElement<'a> {
    CallExpression(Box<'a, CallExpression<'a>>),
    MemberExpression(Box<'a, MemberExpression<'a>>),
}

// Section 13.2 ParenthesizedExpression
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ParenthesizedExpression<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub expression: Expression<'a>,
}

/// Section 14 Statements
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum Statement<'a> {
    // Statements
    BlockStatement(Box<'a, BlockStatement<'a>>),
    BreakStatement(Box<'a, BreakStatement>),
    ContinueStatement(Box<'a, ContinueStatement>),
    DebuggerStatement(Box<'a, DebuggerStatement>),
    DoWhileStatement(Box<'a, DoWhileStatement<'a>>),
    EmptyStatement(Box<'a, EmptyStatement>),
    ExpressionStatement(Box<'a, ExpressionStatement<'a>>),
    ForInStatement(Box<'a, ForInStatement<'a>>),
    ForOfStatement(Box<'a, ForOfStatement<'a>>),
    ForStatement(Box<'a, ForStatement<'a>>),
    IfStatement(Box<'a, IfStatement<'a>>),
    LabeledStatement(Box<'a, LabeledStatement<'a>>),
    ReturnStatement(Box<'a, ReturnStatement<'a>>),
    SwitchStatement(Box<'a, SwitchStatement<'a>>),
    ThrowStatement(Box<'a, ThrowStatement<'a>>),
    TryStatement(Box<'a, TryStatement<'a>>),
    WhileStatement(Box<'a, WhileStatement<'a>>),
    WithStatement(Box<'a, WithStatement<'a>>),

    ModuleDeclaration(Box<'a, ModuleDeclaration<'a>>),
    Declaration(Declaration<'a>),
}

/// Section 11.2.1 Directive Prologue
#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type", rename = "ExpressionStatement")]
pub struct Directive<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub expression: StringLiteral,
    // directives should always use the unescaped raw string
    pub directive: &'a str,
}

/// Section 14.2 Block Statement
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct BlockStatement<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub body: Vec<'a, Statement<'a>>,
}

/// Section 14.3 Declarations and the Variable Statement
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum Declaration<'a> {
    VariableDeclaration(Box<'a, VariableDeclaration<'a>>),
    FunctionDeclaration(Box<'a, Function<'a>>),
    ClassDeclaration(Box<'a, Class<'a>>),

    TSTypeAliasDeclaration(Box<'a, TSTypeAliasDeclaration<'a>>),
    TSInterfaceDeclaration(Box<'a, TSInterfaceDeclaration<'a>>),
    TSEnumDeclaration(Box<'a, TSEnumDeclaration<'a>>),
    TSModuleDeclaration(Box<'a, TSModuleDeclaration<'a>>),
    TSImportEqualsDeclaration(Box<'a, TSImportEqualsDeclaration<'a>>),
}

impl<'a> Declaration<'a> {
    #[must_use]
    pub fn is_typescript_syntax(&self) -> bool {
        match self {
            Self::VariableDeclaration(_) => false,
            Self::FunctionDeclaration(func) => func.is_typescript_syntax(),
            Self::ClassDeclaration(class) => class.is_declare(),
            _ => true,
        }
    }
}

/// Section 14.3.2 Variable Declaration
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type", rename_all = "camelCase")]
pub struct VariableDeclaration<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub kind: VariableDeclarationKind,
    pub declarations: Vec<'a, VariableDeclarator<'a>>,
    /// Valid Modifiers: `export`, `declare`
    #[serde(skip_serializing_if = "Modifiers::is_none")]
    pub modifiers: Modifiers<'a>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum VariableDeclarationKind {
    Var,
    Const,
    Let,
}

impl VariableDeclarationKind {
    #[must_use]
    pub fn is_const(&self) -> bool {
        matches!(self, Self::Const)
    }

    #[must_use]
    pub fn is_lexical(&self) -> bool {
        matches!(self, Self::Const | Self::Let)
    }
}

impl Display for VariableDeclarationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = match self {
            Self::Var => "var",
            Self::Const => "const",
            Self::Let => "let",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct VariableDeclarator<'a> {
    #[serde(flatten)]
    pub span: Span,
    #[serde(skip)]
    pub kind: VariableDeclarationKind,
    pub id: BindingPattern<'a>,
    pub init: Option<Expression<'a>>,
    pub definite: bool,
}

/// Section 14.4 Empty Statement
#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub struct EmptyStatement {
    #[serde(flatten)]
    pub span: Span,
}

/// Section 14.5 Expression Statement
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ExpressionStatement<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub expression: Expression<'a>,
}

/// Section 14.6 If Statement
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct IfStatement<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub test: Expression<'a>,
    pub consequent: Statement<'a>,
    pub alternate: Option<Statement<'a>>,
}

/// Section 14.7.2 Do-While Statement
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct DoWhileStatement<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub body: Statement<'a>,
    pub test: Expression<'a>,
}

/// Section 14.7.3 While Statement
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct WhileStatement<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub test: Expression<'a>,
    pub body: Statement<'a>,
}

/// Section 14.7.4 For Statement
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ForStatement<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub init: Option<ForStatementInit<'a>>,
    pub test: Option<Expression<'a>>,
    pub update: Option<Expression<'a>>,
    pub body: Statement<'a>,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum ForStatementInit<'a> {
    VariableDeclaration(Box<'a, VariableDeclaration<'a>>),
    Expression(Expression<'a>),
}

/// Section 14.7.5 For-In Statement
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ForInStatement<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub left: ForStatementLeft<'a>,
    pub right: Expression<'a>,
    pub body: Statement<'a>,
}

/// Section 14.7.5 For-Of Statement
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ForOfStatement<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub r#await: bool,
    pub left: ForStatementLeft<'a>,
    pub right: Expression<'a>,
    pub body: Statement<'a>,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum ForStatementLeft<'a> {
    VariableDeclaration(Box<'a, VariableDeclaration<'a>>),
    AssignmentTarget(AssignmentTarget<'a>),
}

/// Section 14.8 Continue Statement
#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub struct ContinueStatement {
    #[serde(flatten)]
    pub span: Span,
    pub label: Option<LabelIdentifier>,
}

/// Section 14.9 Break Statement
#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub struct BreakStatement {
    #[serde(flatten)]
    pub span: Span,
    pub label: Option<LabelIdentifier>,
}

/// Section 14.10 Return Statement
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ReturnStatement<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub argument: Option<Expression<'a>>,
}

/// Section 14.11 With Statement
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct WithStatement<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub object: Expression<'a>,
    pub body: Statement<'a>,
}

/// Section 14.12 Switch Statement
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct SwitchStatement<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub discriminant: Expression<'a>,
    pub cases: Vec<'a, SwitchCase<'a>>,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct SwitchCase<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub test: Option<Expression<'a>>,
    pub consequent: Vec<'a, Statement<'a>>,
}

/// Section 14.13 Labelled Statement
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct LabeledStatement<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub label: LabelIdentifier,
    pub body: Statement<'a>,
}

/// Section 14.14 Throw Statement
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ThrowStatement<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub argument: Expression<'a>,
}

/// Section 14.15 Try Statement
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct TryStatement<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub block: Box<'a, BlockStatement<'a>>,
    pub handler: Option<Box<'a, CatchClause<'a>>>,
    pub finalizer: Option<Box<'a, BlockStatement<'a>>>,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct CatchClause<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub param: Option<BindingPattern<'a>>,
    pub body: Box<'a, BlockStatement<'a>>,
}

/// Section 14.16 Debugger Statement
#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub struct DebuggerStatement {
    #[serde(flatten)]
    pub span: Span,
}

/// Section 14.3.3 Destructuring Binding Patterns
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct BindingPattern<'a> {
    #[serde(flatten)]
    pub kind: BindingPatternKind<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<Box<'a, TSTypeAnnotation<'a>>>,
    pub optional: bool,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum BindingPatternKind<'a> {
    BindingIdentifier(Box<'a, BindingIdentifier>),
    ObjectPattern(Box<'a, ObjectPattern<'a>>),
    ArrayPattern(Box<'a, ArrayPattern<'a>>),
    RestElement(Box<'a, RestElement<'a>>),
    AssignmentPattern(Box<'a, AssignmentPattern<'a>>),
}

impl<'a> BindingPatternKind<'a> {
    #[must_use]
    pub fn is_destructuring_pattern(&self) -> bool {
        matches!(self, Self::ObjectPattern(_) | Self::ArrayPattern(_))
    }

    #[must_use]
    pub fn is_rest_element(&self) -> bool {
        matches!(self, Self::RestElement(_))
    }
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct AssignmentPattern<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub left: BindingPattern<'a>,
    pub right: Expression<'a>,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ObjectPattern<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub properties: Vec<'a, ObjectPatternProperty<'a>>,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum ObjectPatternProperty<'a> {
    Property(Box<'a, Property<'a>>),
    RestElement(Box<'a, RestElement<'a>>),
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ArrayPattern<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub elements: Vec<'a, Option<BindingPattern<'a>>>,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct RestElement<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub argument: BindingPattern<'a>,
}

/// Section 15.2 Function Definitions
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct Function<'a> {
    pub r#type: FunctionType,
    #[serde(flatten)]
    pub span: Span,
    pub id: Option<BindingIdentifier>,
    pub expression: bool,
    pub generator: bool,
    pub r#async: bool,
    pub params: Box<'a, FormalParameters<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Box<'a, FunctionBody<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<Box<'a, TSTypeParameterDeclaration<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_type: Option<Box<'a, TSTypeAnnotation<'a>>>,
    /// Valid modifiers: `export`, `default`, `async`
    #[serde(skip_serializing_if = "Modifiers::is_none")]
    pub modifiers: Modifiers<'a>,
}

impl<'a> Function<'a> {
    #[must_use]
    pub fn is_typescript_syntax(&self) -> bool {
        self.modifiers.contains(ModifierKind::Declare) || self.body.is_none()
    }

    #[must_use]
    pub fn is_expression(&self) -> bool {
        self.r#type == FunctionType::FunctionExpression
    }

    #[must_use]
    pub fn is_function_declaration(&self) -> bool {
        matches!(self.r#type, FunctionType::FunctionDeclaration)
    }

    #[must_use]
    pub fn is_ts_declare_function(&self) -> bool {
        matches!(self.r#type, FunctionType::TSDeclareFunction)
    }

    #[must_use]
    pub fn is_declaration(&self) -> bool {
        matches!(self.r#type, FunctionType::FunctionDeclaration | FunctionType::TSDeclareFunction)
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
pub enum FunctionType {
    FunctionDeclaration,
    FunctionExpression,
    TSDeclareFunction,
}

#[derive(Debug, PartialEq, Hash)]
pub struct FormalParameters<'a> {
    pub span: Span,
    pub kind: FormalParameterKind,
    pub items: Vec<'a, FormalParameter<'a>>,
}

#[derive(Debug, PartialEq, Hash, Serialize)]
pub struct FormalParameter<'a> {
    #[serde(flatten)]
    pub span: Span,
    #[serde(flatten)]
    pub pattern: BindingPattern<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility: Option<TSAccessibility>,
    pub readonly: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub decorators: Vec<'a, Decorator<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormalParameterKind {
    /// https://tc39.es/ecma262/#prod-FormalParameters
    FormalParameter,
    /// https://tc39.es/ecma262/#prod-UniqueFormalParameters
    UniqueFormalParameters,
    /// https://tc39.es/ecma262/#prod-ArrowFormalParameters
    ArrowFormalParameters,
    /// Part of TypeScript type signatures
    Signature,
}

impl<'a> FormalParameters<'a> {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[derive(Debug, PartialEq, Hash)]
pub struct FunctionBody<'a> {
    pub span: Span,
    pub directives: Vec<'a, Directive<'a>>,
    pub statements: Vec<'a, Statement<'a>>,
}

impl<'a> FunctionBody<'a> {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.directives.is_empty() && self.statements.is_empty()
    }
}

/// Section 15.3 Arrow Function Definitions
#[derive(Debug, PartialEq, Hash)]
pub struct ArrowExpression<'a> {
    pub span: Span,
    pub expression: bool,
    pub generator: bool,
    pub r#async: bool,
    pub params: Box<'a, FormalParameters<'a>>, // UniqueFormalParameters in spec
    pub body: Box<'a, FunctionBody<'a>>,

    pub type_parameters: Option<Box<'a, TSTypeParameterDeclaration<'a>>>,
    pub return_type: Option<Box<'a, TSTypeAnnotation<'a>>>,
}

/// Section 15.5 Generator Function Definitions
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct YieldExpression<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub delegate: bool,
    pub argument: Option<Expression<'a>>,
}

/// Section 15.7 Class Definitions
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Class<'a> {
    pub r#type: ClassType,
    #[serde(flatten)]
    pub span: Span,
    pub id: Option<BindingIdentifier>,
    pub super_class: Option<Expression<'a>>,
    pub body: Box<'a, ClassBody<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<Box<'a, TSTypeParameterDeclaration<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub super_type_parameters: Option<Box<'a, TSTypeParameterInstantiation<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implements: Option<Vec<'a, Box<'a, TSClassImplements<'a>>>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub decorators: Vec<'a, Decorator<'a>>,
    /// Valid Modifiers: `export`, `abstract`
    #[serde(skip_serializing_if = "Modifiers::is_none")]
    pub modifiers: Modifiers<'a>,
}

impl<'a> Class<'a> {
    #[must_use]
    pub fn is_expression(&self) -> bool {
        self.r#type == ClassType::ClassExpression
    }

    #[must_use]
    pub fn is_declaration(&self) -> bool {
        self.r#type == ClassType::ClassDeclaration
    }

    #[must_use]
    pub fn is_declare(&self) -> bool {
        self.modifiers.contains(ModifierKind::Declare)
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
pub enum ClassType {
    ClassDeclaration,
    ClassExpression,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ClassBody<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub body: Vec<'a, ClassElement<'a>>,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum ClassElement<'a> {
    StaticBlock(Box<'a, StaticBlock<'a>>),
    MethodDefinition(Box<'a, MethodDefinition<'a>>),
    PropertyDefinition(Box<'a, PropertyDefinition<'a>>),
    AccessorProperty(Box<'a, AccessorProperty<'a>>),
    TSAbstractMethodDefinition(Box<'a, TSAbstractMethodDefinition<'a>>),
    TSAbstractPropertyDefinition(Box<'a, TSAbstractPropertyDefinition<'a>>),
    TSIndexSignature(Box<'a, TSIndexSignature<'a>>),
}

impl<'a> ClassElement<'a> {
    #[must_use]
    pub fn r#static(&self) -> bool {
        match self {
            Self::TSIndexSignature(_) | Self::StaticBlock(_) => false,
            Self::MethodDefinition(def) => def.r#static,
            Self::PropertyDefinition(def) => def.r#static,
            Self::AccessorProperty(def) => def.r#static,
            Self::TSAbstractMethodDefinition(def) => def.method_definition.r#static,
            Self::TSAbstractPropertyDefinition(def) => def.property_definition.r#static,
        }
    }

    #[must_use]
    pub fn computed(&self) -> bool {
        match self {
            Self::TSIndexSignature(_) | Self::StaticBlock(_) => false,
            Self::MethodDefinition(def) => def.computed,
            Self::PropertyDefinition(def) => def.computed,
            Self::AccessorProperty(def) => def.computed,
            Self::TSAbstractMethodDefinition(def) => def.method_definition.computed,
            Self::TSAbstractPropertyDefinition(def) => def.property_definition.computed,
        }
    }

    #[must_use]
    pub fn method_definition_kind(&self) -> Option<MethodDefinitionKind> {
        match self {
            Self::TSIndexSignature(_)
            | Self::StaticBlock(_)
            | Self::PropertyDefinition(_)
            | Self::AccessorProperty(_) => None,
            Self::MethodDefinition(def) => Some(def.kind),
            Self::TSAbstractMethodDefinition(def) => Some(def.method_definition.kind),
            Self::TSAbstractPropertyDefinition(_def) => None,
        }
    }

    #[must_use]
    pub fn property_key(&self) -> Option<&PropertyKey<'a>> {
        match self {
            Self::TSIndexSignature(_) | Self::StaticBlock(_) => None,
            Self::MethodDefinition(def) => Some(&def.key),
            Self::PropertyDefinition(def) => Some(&def.key),
            Self::AccessorProperty(def) => Some(&def.key),
            Self::TSAbstractMethodDefinition(def) => Some(&def.method_definition.key),
            Self::TSAbstractPropertyDefinition(def) => Some(&def.property_definition.key),
        }
    }

    #[must_use]
    pub fn static_name(&self) -> Option<Atom> {
        match self {
            Self::TSIndexSignature(_) | Self::StaticBlock(_) => None,
            Self::MethodDefinition(def) => def.key.static_name(),
            Self::PropertyDefinition(def) => def.key.static_name(),
            Self::AccessorProperty(def) => def.key.static_name(),
            Self::TSAbstractMethodDefinition(_def) => None,
            Self::TSAbstractPropertyDefinition(_def) => None,
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
#[allow(clippy::struct_excessive_bools)]
pub struct MethodDefinition<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub key: PropertyKey<'a>,
    pub value: Box<'a, Function<'a>>, // FunctionExpression
    pub kind: MethodDefinitionKind,
    pub computed: bool,
    pub r#static: bool,
    pub r#override: bool,
    pub optional: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility: Option<TSAccessibility>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub decorators: Vec<'a, Decorator<'a>>,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type", rename_all = "camelCase")]
#[allow(clippy::struct_excessive_bools)]
pub struct PropertyDefinition<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub key: PropertyKey<'a>,
    pub value: Option<Expression<'a>>,
    pub computed: bool,
    pub r#static: bool,
    pub declare: bool,
    pub r#override: bool,
    pub optional: bool,
    pub definite: bool,
    pub readonly: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_annotation: Option<Box<'a, TSTypeAnnotation<'a>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accessibility: Option<TSAccessibility>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub decorators: Vec<'a, Decorator<'a>>,
}

#[derive(Debug, Clone, Copy, Serialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum MethodDefinitionKind {
    Constructor,
    Method,
    Get,
    Set,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub struct PrivateIdentifier {
    #[serde(flatten)]
    pub span: Span,
    pub name: Atom,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct StaticBlock<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub body: Vec<'a, Statement<'a>>,
}

/// Section 16.2.2 Imports
#[derive(Debug, Serialize, PartialEq, Hash)]
pub struct ModuleDeclaration<'a> {
    #[serde(flatten)]
    pub span: Span,
    #[serde(flatten)]
    pub kind: ModuleDeclarationKind<'a>,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum ModuleDeclarationKind<'a> {
    ImportDeclaration(Box<'a, ImportDeclaration<'a>>),
    ExportAllDeclaration(Box<'a, ExportAllDeclaration<'a>>),
    ExportDefaultDeclaration(Box<'a, ExportDefaultDeclaration<'a>>),
    ExportNamedDeclaration(Box<'a, ExportNamedDeclaration<'a>>),

    TSExportAssignment(Box<'a, TSExportAssignment<'a>>),
    TSNamespaceExportDeclaration(Box<'a, TSNamespaceExportDeclaration>),
}

impl<'a> ModuleDeclarationKind<'a> {
    #[must_use]
    pub fn is_export(&self) -> bool {
        matches!(
            self,
            Self::ExportAllDeclaration(_)
                | Self::ExportDefaultDeclaration(_)
                | Self::ExportNamedDeclaration(_)
                | Self::TSExportAssignment(_)
                | Self::TSNamespaceExportDeclaration(_)
        )
    }
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct AccessorProperty<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub key: PropertyKey<'a>,
    pub value: Option<Expression<'a>>,
    pub computed: bool,
    pub r#static: bool,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ImportExpression<'a> {
    #[serde(flatten)]
    pub span: Span,
    pub source: Expression<'a>,
    pub arguments: Vec<'a, Expression<'a>>,
}

#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type", rename_all = "camelCase")]
pub struct ImportDeclaration<'a> {
    pub specifiers: Vec<'a, ImportDeclarationSpecifier>,
    pub source: StringLiteral,
    pub assertions: Option<Vec<'a, ImportAttribute>>, // Some(vec![]) for empty assertion
    pub import_kind: Option<ImportOrExportKind>,      // `import type { foo } from 'bar'`
}

#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum ImportDeclarationSpecifier {
    ImportSpecifier(ImportSpecifier),
    ImportDefaultSpecifier(ImportDefaultSpecifier),
    ImportNamespaceSpecifier(ImportNamespaceSpecifier),
}

// import {imported} from "source"
// import {imported as local} from "source"
#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub struct ImportSpecifier {
    #[serde(flatten)]
    pub span: Span,
    pub imported: ModuleExportName,
    pub local: BindingIdentifier,
}

// import local from "source"
#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub struct ImportDefaultSpecifier {
    #[serde(flatten)]
    pub span: Span,
    pub local: BindingIdentifier,
}

// import * as local from "source"
#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub struct ImportNamespaceSpecifier {
    #[serde(flatten)]
    pub span: Span,
    pub local: BindingIdentifier,
}

#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub struct ImportAttribute {
    #[serde(flatten)]
    pub span: Span,
    pub key: ImportAttributeKey,
    pub value: StringLiteral,
}

#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum ImportAttributeKey {
    Identifier(IdentifierName),
    StringLiteral(StringLiteral),
}

impl ImportAttributeKey {
    #[must_use]
    pub fn as_atom(&self) -> Atom {
        match self {
            Self::Identifier(identifier) => identifier.name.clone(),
            Self::StringLiteral(literal) => literal.value.clone(),
        }
    }
}

/// Exports
/// [tc39/ecma262#sec-exports](https://tc39.es/ecma262/#sec-exports)
#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ExportNamedDeclaration<'a> {
    pub declaration: Option<Declaration<'a>>,
    pub specifiers: Vec<'a, ExportSpecifier>,
    pub source: Option<StringLiteral>,
    pub export_kind: Option<ImportOrExportKind>, // `export type { foo }`
}

impl<'a> ExportNamedDeclaration<'a> {
    #[must_use]
    pub fn is_typescript_syntax(&self) -> bool {
        self.export_kind == Some(ImportOrExportKind::Type)
            || self.declaration.as_ref().map_or(false, Declaration::is_typescript_syntax)
    }
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(tag = "type")]
pub struct ExportDefaultDeclaration<'a> {
    pub declaration: ExportDefaultDeclarationKind<'a>,
    pub exported: ModuleExportName, // `default`
}

#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub struct ExportAllDeclaration<'a> {
    pub exported: Option<ModuleExportName>,
    pub source: StringLiteral,
    pub assertions: Option<Vec<'a, ImportAttribute>>, // Some(vec![]) for empty assertion
    pub export_kind: Option<ImportOrExportKind>,      // `export type *`
}

#[derive(Debug, Serialize, PartialEq, Eq, Hash)]
#[serde(tag = "type")]
pub struct ExportSpecifier {
    #[serde(flatten)]
    pub span: Span,
    pub local: ModuleExportName,
    pub exported: ModuleExportName,
}

#[derive(Debug, Serialize, PartialEq, Hash)]
#[serde(untagged)]
pub enum ExportDefaultDeclarationKind<'a> {
    Expression(Expression<'a>),
    FunctionDeclaration(Box<'a, Function<'a>>),
    ClassDeclaration(Box<'a, Class<'a>>),

    TSInterfaceDeclaration(Box<'a, TSInterfaceDeclaration<'a>>),
    TSEnumDeclaration(Box<'a, TSEnumDeclaration<'a>>),
}

impl<'a> ExportDefaultDeclarationKind<'a> {
    #[inline]
    #[must_use]
    pub fn is_typescript_syntax(&self) -> bool {
        match self {
            ExportDefaultDeclarationKind::FunctionDeclaration(func)
                if func.is_typescript_syntax() =>
            {
                true
            }
            ExportDefaultDeclarationKind::TSInterfaceDeclaration(_)
            | ExportDefaultDeclarationKind::TSEnumDeclaration(_) => true,
            _ => false,
        }
    }
}

// es2022: https://github.com/oxc_ast/oxc_ast/blob/master/es2022.md#modules
// https://github.com/tc39/ecma262/pull/2154
// support:
//   import {"\0 any unicode" as foo} from "";
//   export {foo as "\0 any unicode"};
#[derive(Debug, Clone, Serialize, PartialEq, Eq, Hash)]
#[serde(untagged)]
pub enum ModuleExportName {
    Identifier(IdentifierName),
    StringLiteral(StringLiteral),
}

impl Display for ModuleExportName {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = match self {
            Self::Identifier(identifier) => identifier.name.to_string(),
            Self::StringLiteral(literal) => literal.value.to_string(),
        };
        write!(f, "{s}")
    }
}

impl ModuleExportName {
    #[must_use]
    pub fn name(&self) -> &Atom {
        match self {
            Self::Identifier(identifier) => &identifier.name,
            Self::StringLiteral(literal) => &literal.value,
        }
    }
}
