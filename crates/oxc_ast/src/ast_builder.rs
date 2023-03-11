//! AST builder for creating AST spans

#![allow(clippy::unused_self, clippy::missing_const_for_fn, clippy::too_many_arguments)]

use oxc_allocator::{Allocator, Box, Vec};

#[allow(clippy::wildcard_imports)]
use crate::{ast::*, Atom, SourceType, Span};

pub struct AstBuilder<'a> {
    pub allocator: &'a Allocator,
}

impl<'a> AstBuilder<'a> {
    pub fn new(allocator: &'a Allocator) -> Self {
        Self { allocator }
    }

    #[inline]
    pub fn alloc<T>(&self, value: T) -> Box<'a, T> {
        Box(self.allocator.alloc(value))
    }

    #[must_use]
    #[inline]
    pub fn new_vec<T>(&self) -> Vec<'a, T> {
        Vec::new_in(self.allocator)
    }

    #[must_use]
    pub fn new_vec_with_capacity<T>(&self, capacity: usize) -> Vec<'a, T> {
        Vec::with_capacity_in(capacity, self.allocator)
    }

    #[must_use]
    pub fn new_vec_single<T>(&self, value: T) -> Vec<'a, T> {
        let mut vec = self.new_vec_with_capacity(1);
        vec.push(value);
        vec
    }

    #[must_use]
    #[inline]
    pub fn new_str(&self, value: &str) -> &'a str {
        &*self.allocator.alloc_str(value)
    }

    #[must_use]
    #[inline]
    pub fn program(
        &self,
        span: Span,
        directives: Vec<'a, Directive<'a>>,
        body: Vec<'a, Statement<'a>>,
        source_type: SourceType,
    ) -> Program<'a> {
        Program { span, directives, body, source_type }
    }

    /* ---------- Literals ---------- */

    #[must_use]
    #[inline]
    pub fn literal_string_expression(&self, literal: StringLiteral) -> Expression<'a> {
        Expression::StringLiteral(self.alloc(literal))
    }

    #[must_use]
    #[inline]
    pub fn literal_boolean_expression(&self, literal: BooleanLiteral) -> Expression<'a> {
        Expression::BooleanLiteral(self.alloc(literal))
    }

    #[must_use]
    #[inline]
    pub fn literal_null_expression(&self, literal: NullLiteral) -> Expression<'a> {
        Expression::NullLiteral(self.alloc(literal))
    }

    #[must_use]
    #[inline]
    pub fn literal_regexp_expression(&self, literal: RegExpLiteral) -> Expression<'a> {
        Expression::RegExpLiteral(self.alloc(literal))
    }

    #[must_use]
    #[inline]
    pub fn literal_number_expression(&self, literal: NumberLiteral<'a>) -> Expression<'a> {
        Expression::NumberLiteral(self.alloc(literal))
    }

    #[must_use]
    #[inline]
    pub fn literal_bigint_expression(&self, literal: BigintLiteral) -> Expression<'a> {
        Expression::BigintLiteral(self.alloc(literal))
    }

    /* ---------- Identifiers ---------- */

    #[must_use]
    #[inline]
    pub fn identifier_expression(&self, identifier: IdentifierReference) -> Expression<'a> {
        Expression::Identifier(self.alloc(identifier))
    }

    /* ---------- Statements ---------- */

    #[must_use]
    #[inline]
    pub fn directive(
        &self,
        span: Span,
        expression: StringLiteral,
        directive: &'a str,
    ) -> Directive<'a> {
        Directive { span, expression, directive }
    }

    #[must_use]
    #[inline]
    pub fn block(&self, span: Span, body: Vec<'a, Statement<'a>>) -> Box<'a, BlockStatement<'a>> {
        self.alloc(BlockStatement { span, body })
    }

    #[must_use]
    #[inline]
    pub fn block_statement(&self, block: Box<'a, BlockStatement<'a>>) -> Statement<'a> {
        Statement::BlockStatement(
            self.alloc(BlockStatement { span: block.span, body: Box::into_inner(block).body }),
        )
    }

    #[must_use]
    #[inline]
    pub fn break_statement(&self, span: Span, label: Option<LabelIdentifier>) -> Statement<'a> {
        Statement::BreakStatement(self.alloc(BreakStatement { span, label }))
    }

    #[must_use]
    #[inline]
    pub fn continue_statement(&self, span: Span, label: Option<LabelIdentifier>) -> Statement<'a> {
        Statement::ContinueStatement(self.alloc(ContinueStatement { span, label }))
    }

    #[must_use]
    #[inline]
    pub fn debugger_statement(&self, span: Span) -> Statement<'a> {
        Statement::DebuggerStatement(self.alloc(DebuggerStatement { span }))
    }

    #[must_use]
    #[inline]
    pub fn do_while_statement(
        &self,
        span: Span,
        body: Statement<'a>,
        test: Expression<'a>,
    ) -> Statement<'a> {
        Statement::DoWhileStatement(self.alloc(DoWhileStatement { span, body, test }))
    }

    #[must_use]
    #[inline]
    pub fn empty_statement(&self, span: Span) -> Statement<'a> {
        Statement::EmptyStatement(self.alloc(EmptyStatement { span }))
    }

    #[must_use]
    #[inline]
    pub fn expression_statement(&self, span: Span, expression: Expression<'a>) -> Statement<'a> {
        Statement::ExpressionStatement(self.alloc(ExpressionStatement { span, expression }))
    }

    #[must_use]
    #[inline]
    pub fn for_in_statement(
        &self,
        span: Span,
        left: ForStatementLeft<'a>,
        right: Expression<'a>,
        body: Statement<'a>,
    ) -> Statement<'a> {
        Statement::ForInStatement(self.alloc(ForInStatement { span, left, right, body }))
    }

    #[must_use]
    #[inline]
    pub fn for_of_statement(
        &self,
        span: Span,
        r#await: bool,
        left: ForStatementLeft<'a>,
        right: Expression<'a>,
        body: Statement<'a>,
    ) -> Statement<'a> {
        Statement::ForOfStatement(self.alloc(ForOfStatement { span, r#await, left, right, body }))
    }

    #[must_use]
    #[inline]
    pub fn for_statement(
        &self,
        span: Span,
        init: Option<ForStatementInit<'a>>,
        test: Option<Expression<'a>>,
        update: Option<Expression<'a>>,
        body: Statement<'a>,
    ) -> Statement<'a> {
        Statement::ForStatement(self.alloc(ForStatement { span, init, test, update, body }))
    }

    #[must_use]
    #[inline]
    pub fn if_statement(
        &self,
        span: Span,
        test: Expression<'a>,
        consequent: Statement<'a>,
        alternate: Option<Statement<'a>>,
    ) -> Statement<'a> {
        Statement::IfStatement(self.alloc(IfStatement { span, test, consequent, alternate }))
    }

    #[must_use]
    #[inline]
    pub fn labeled_statement(
        &self,
        span: Span,
        label: LabelIdentifier,
        body: Statement<'a>,
    ) -> Statement<'a> {
        Statement::LabeledStatement(self.alloc(LabeledStatement { span, label, body }))
    }

    #[must_use]
    #[inline]
    pub fn return_statement(&self, span: Span, argument: Option<Expression<'a>>) -> Statement<'a> {
        Statement::ReturnStatement(self.alloc(ReturnStatement { span, argument }))
    }

    #[must_use]
    #[inline]
    pub fn switch_statement(
        &self,
        span: Span,
        discriminant: Expression<'a>,
        cases: Vec<'a, SwitchCase<'a>>,
    ) -> Statement<'a> {
        Statement::SwitchStatement(self.alloc(SwitchStatement { span, discriminant, cases }))
    }

    #[must_use]
    #[inline]
    pub fn switch_case(
        &self,
        span: Span,
        test: Option<Expression<'a>>,
        consequent: Vec<'a, Statement<'a>>,
    ) -> SwitchCase<'a> {
        SwitchCase { span, test, consequent }
    }

    #[must_use]
    #[inline]
    pub fn throw_statement(&self, span: Span, argument: Expression<'a>) -> Statement<'a> {
        Statement::ThrowStatement(self.alloc(ThrowStatement { span, argument }))
    }

    #[must_use]
    #[inline]
    pub fn try_statement(
        &self,
        span: Span,
        block: Box<'a, BlockStatement<'a>>,
        handler: Option<Box<'a, CatchClause<'a>>>,
        finalizer: Option<Box<'a, BlockStatement<'a>>>,
    ) -> Statement<'a> {
        Statement::TryStatement(self.alloc(TryStatement { span, block, handler, finalizer }))
    }

    #[must_use]
    #[inline]
    pub fn catch_clause(
        &self,
        span: Span,
        param: Option<BindingPattern<'a>>,
        body: Box<'a, BlockStatement<'a>>,
    ) -> Box<'a, CatchClause<'a>> {
        self.alloc(CatchClause { span, param, body })
    }

    #[must_use]
    #[inline]
    pub fn while_statement(
        &self,
        span: Span,
        test: Expression<'a>,
        body: Statement<'a>,
    ) -> Statement<'a> {
        Statement::WhileStatement(self.alloc(WhileStatement { span, test, body }))
    }

    #[must_use]
    #[inline]
    pub fn with_statement(
        &self,
        span: Span,
        object: Expression<'a>,
        body: Statement<'a>,
    ) -> Statement<'a> {
        Statement::WithStatement(self.alloc(WithStatement { span, object, body }))
    }

    /* ---------- Expressions ---------- */

    #[must_use]
    #[inline]
    pub fn super_(&self, span: Span) -> Expression<'a> {
        Expression::Super(self.alloc(Super { span }))
    }

    #[must_use]
    #[inline]
    pub fn meta_property(
        &self,
        span: Span,
        meta: IdentifierName,
        property: IdentifierName,
    ) -> Expression<'a> {
        Expression::MetaProperty(self.alloc(MetaProperty { span, meta, property }))
    }

    #[must_use]
    #[inline]
    pub fn array_expression(
        &self,
        span: Span,
        elements: Vec<'a, Option<Argument<'a>>>,
        trailing_comma: Option<Span>,
    ) -> Expression<'a> {
        Expression::ArrayExpression(self.alloc(ArrayExpression { span, elements, trailing_comma }))
    }

    #[must_use]
    #[inline]
    pub fn arrow_expression(
        &self,
        span: Span,
        expression: bool,
        generator: bool,
        r#async: bool,
        params: Box<'a, FormalParameters<'a>>,
        body: Box<'a, FunctionBody<'a>>,
        type_parameters: Option<Box<'a, TSTypeParameterDeclaration<'a>>>,
        return_type: Option<Box<'a, TSTypeAnnotation<'a>>>,
    ) -> Expression<'a> {
        Expression::ArrowFunctionExpression(self.alloc(ArrowExpression {
            span,
            expression,
            generator,
            r#async,
            params,
            body,
            type_parameters,
            return_type,
        }))
    }

    #[must_use]
    #[inline]
    pub fn assignment_expression(
        &self,
        span: Span,
        operator: AssignmentOperator,
        left: AssignmentTarget<'a>,
        right: Expression<'a>,
    ) -> Expression<'a> {
        Expression::AssignmentExpression(self.alloc(AssignmentExpression {
            span,
            operator,
            left,
            right,
        }))
    }

    #[must_use]
    #[inline]
    pub fn await_expression(&self, span: Span, argument: Expression<'a>) -> Expression<'a> {
        Expression::AwaitExpression(self.alloc(AwaitExpression { span, argument }))
    }

    #[must_use]
    #[inline]
    pub fn binary_expression(
        &self,
        span: Span,
        left: Expression<'a>,
        operator: BinaryOperator,
        right: Expression<'a>,
    ) -> Expression<'a> {
        Expression::BinaryExpression(self.alloc(BinaryExpression { span, left, operator, right }))
    }

    #[must_use]
    #[inline]
    pub fn call_expression(
        &self,
        span: Span,
        callee: Expression<'a>,
        arguments: Vec<'a, Argument<'a>>,
        optional: bool, // for optional chaining
        type_parameters: Option<Box<'a, TSTypeParameterInstantiation<'a>>>,
    ) -> Expression<'a> {
        Expression::CallExpression(self.alloc(CallExpression {
            span,
            callee,
            arguments,
            optional,
            type_parameters,
        }))
    }

    #[must_use]
    #[inline]
    pub fn chain_expression(&self, span: Span, expression: ChainElement<'a>) -> Expression<'a> {
        Expression::ChainExpression(self.alloc(ChainExpression { span, expression }))
    }

    #[must_use]
    #[inline]
    pub fn class_expression(&self, class: Box<'a, Class<'a>>) -> Expression<'a> {
        Expression::ClassExpression(class)
    }

    #[must_use]
    #[inline]
    pub fn conditional_expression(
        &self,
        span: Span,
        test: Expression<'a>,
        consequent: Expression<'a>,
        alternate: Expression<'a>,
    ) -> Expression<'a> {
        Expression::ConditionalExpression(self.alloc(ConditionalExpression {
            span,
            test,
            consequent,
            alternate,
        }))
    }

    #[must_use]
    #[inline]
    pub fn function_expression(&self, function: Box<'a, Function<'a>>) -> Expression<'a> {
        Expression::FunctionExpression(function)
    }

    #[must_use]
    #[inline]
    pub fn import_expression(
        &self,
        span: Span,
        source: Expression<'a>,
        arguments: Vec<'a, Expression<'a>>,
    ) -> Expression<'a> {
        Expression::ImportExpression(self.alloc(ImportExpression { span, source, arguments }))
    }

    #[must_use]
    #[inline]
    pub fn logical_expression(
        &self,
        span: Span,
        left: Expression<'a>,
        operator: LogicalOperator,
        right: Expression<'a>,
    ) -> Expression<'a> {
        Expression::LogicalExpression(self.alloc(LogicalExpression { span, left, operator, right }))
    }

    #[must_use]
    #[inline]
    pub fn computed_member_expression(
        &self,
        span: Span,
        object: Expression<'a>,
        expression: Expression<'a>,
        optional: bool, // for optional chaining
    ) -> Expression<'a> {
        Expression::MemberExpression(self.alloc({
            MemberExpression::ComputedMemberExpression(ComputedMemberExpression {
                span,
                object,
                expression,
                optional,
            })
        }))
    }

    #[must_use]
    #[inline]
    pub fn static_member_expression(
        &self,
        span: Span,
        object: Expression<'a>,
        property: IdentifierName,
        optional: bool, // for optional chaining
    ) -> Expression<'a> {
        Expression::MemberExpression(self.alloc({
            MemberExpression::StaticMemberExpression(StaticMemberExpression {
                span,
                object,
                property,
                optional,
            })
        }))
    }

    #[must_use]
    #[inline]
    pub fn private_field_expression(
        &self,
        span: Span,
        object: Expression<'a>,
        field: PrivateIdentifier,
        optional: bool,
    ) -> Expression<'a> {
        Expression::MemberExpression(self.alloc({
            MemberExpression::PrivateFieldExpression(PrivateFieldExpression {
                span,
                object,
                field,
                optional,
            })
        }))
    }

    #[must_use]
    #[inline]
    pub fn new_expression(
        &self,
        span: Span,
        callee: Expression<'a>,
        arguments: Vec<'a, Argument<'a>>,
        type_parameters: Option<Box<'a, TSTypeParameterInstantiation<'a>>>,
    ) -> Expression<'a> {
        Expression::NewExpression(self.alloc(NewExpression {
            span,
            callee,
            arguments,
            type_parameters,
        }))
    }

    #[must_use]
    #[inline]
    pub fn object_expression(
        &self,
        span: Span,
        properties: Vec<'a, ObjectProperty<'a>>,
        trailing_comma: Option<Span>,
    ) -> Expression<'a> {
        Expression::ObjectExpression(self.alloc(ObjectExpression {
            span,
            properties,
            trailing_comma,
        }))
    }

    #[must_use]
    #[inline]
    pub fn parenthesized_expression(
        &self,
        span: Span,
        expression: Expression<'a>,
    ) -> Expression<'a> {
        Expression::ParenthesizedExpression(
            self.alloc(ParenthesizedExpression { span, expression }),
        )
    }

    #[must_use]
    #[inline]
    pub fn sequence_expression(
        &self,
        span: Span,
        expressions: Vec<'a, Expression<'a>>,
    ) -> Expression<'a> {
        Expression::SequenceExpression(self.alloc(SequenceExpression { span, expressions }))
    }

    #[must_use]
    #[inline]
    pub fn tagged_template_expression(
        &self,
        span: Span,
        tag: Expression<'a>,
        quasi: TemplateLiteral<'a>,
        type_parameters: Option<Box<'a, TSTypeParameterInstantiation<'a>>>,
    ) -> Expression<'a> {
        Expression::TaggedTemplateExpression(self.alloc(TaggedTemplateExpression {
            span,
            tag,
            quasi,
            type_parameters,
        }))
    }

    #[must_use]
    #[inline]
    pub fn template_literal_expression(
        &self,
        template_literal: TemplateLiteral<'a>,
    ) -> Expression<'a> {
        Expression::TemplateLiteral(self.alloc(template_literal))
    }

    #[must_use]
    #[inline]
    pub fn this_expression(&self, span: Span) -> Expression<'a> {
        Expression::ThisExpression(self.alloc(ThisExpression { span }))
    }

    #[must_use]
    #[inline]
    pub fn unary_expression(
        &self,
        span: Span,
        operator: UnaryOperator,
        prefix: bool,
        argument: Expression<'a>,
    ) -> Expression<'a> {
        Expression::UnaryExpression(self.alloc(UnaryExpression {
            span,
            operator,
            prefix,
            argument,
        }))
    }

    #[must_use]
    #[inline]
    pub fn update_expression(
        &self,
        span: Span,
        operator: UpdateOperator,
        prefix: bool,
        argument: SimpleAssignmentTarget<'a>,
    ) -> Expression<'a> {
        Expression::UpdateExpression(self.alloc(UpdateExpression {
            span,
            operator,
            prefix,
            argument,
        }))
    }

    #[must_use]
    #[inline]
    pub fn yield_expression(
        &self,
        span: Span,
        delegate: bool,
        argument: Option<Expression<'a>>,
    ) -> Expression<'a> {
        Expression::YieldExpression(self.alloc(YieldExpression { span, delegate, argument }))
    }

    /* ---------- Functions ---------- */
    #[must_use]
    #[inline]
    pub fn function_declaration(&self, func: Box<'a, Function<'a>>) -> Statement<'a> {
        Statement::Declaration(Declaration::FunctionDeclaration(func))
    }

    #[must_use]
    #[inline]
    pub fn formal_parameters(
        &self,
        span: Span,
        kind: FormalParameterKind,
        items: Vec<'a, FormalParameter<'a>>,
    ) -> Box<'a, FormalParameters<'a>> {
        self.alloc(FormalParameters { span, kind, items })
    }

    #[must_use]
    #[inline]
    pub fn formal_parameter(
        &self,
        span: Span,
        pattern: BindingPattern<'a>,
        accessibility: Option<TSAccessibility>,
        readonly: bool,
        decorators: Vec<'a, Decorator<'a>>,
    ) -> FormalParameter<'a> {
        FormalParameter { span, pattern, accessibility, readonly, decorators }
    }

    #[must_use]
    #[inline]
    #[allow(clippy::fn_params_excessive_bools)]
    pub fn function(
        &self,
        r#type: FunctionType,
        span: Span,
        id: Option<BindingIdentifier>,
        expression: bool,
        generator: bool,
        r#async: bool,
        params: Box<'a, FormalParameters<'a>>,
        body: Option<Box<'a, FunctionBody<'a>>>,
        type_parameters: Option<Box<'a, TSTypeParameterDeclaration<'a>>>,
        return_type: Option<Box<'a, TSTypeAnnotation<'a>>>,
        modifiers: Modifiers<'a>,
    ) -> Box<'a, Function<'a>> {
        self.alloc(Function {
            r#type,
            span,
            id,
            expression,
            generator,
            r#async,
            params,
            body,
            type_parameters,
            return_type,
            modifiers,
        })
    }

    #[must_use]
    #[inline]
    pub fn function_body(
        &self,
        span: Span,
        directives: Vec<'a, Directive<'a>>,
        statements: Vec<'a, Statement<'a>>,
    ) -> Box<'a, FunctionBody<'a>> {
        self.alloc(FunctionBody { span, directives, statements })
    }

    /* ---------- Class ---------- */

    #[must_use]
    #[inline]
    pub fn class(
        &self,
        r#type: ClassType,
        span: Span,
        id: Option<BindingIdentifier>,
        super_class: Option<Expression<'a>>,
        body: Box<'a, ClassBody<'a>>,
        type_parameters: Option<Box<'a, TSTypeParameterDeclaration<'a>>>,
        super_type_parameters: Option<Box<'a, TSTypeParameterInstantiation<'a>>>,
        implements: Option<Vec<'a, Box<'a, TSClassImplements<'a>>>>,
        decorators: Vec<'a, Decorator<'a>>,
        modifiers: Modifiers<'a>,
    ) -> Box<'a, Class<'a>> {
        self.alloc(Class {
            r#type,
            span,
            id,
            super_class,
            body,
            type_parameters,
            super_type_parameters,
            implements,
            decorators,
            modifiers,
        })
    }

    #[must_use]
    #[inline]
    pub fn class_body(
        &self,
        span: Span,
        body: Vec<'a, ClassElement<'a>>,
    ) -> Box<'a, ClassBody<'a>> {
        self.alloc(ClassBody { span, body })
    }

    #[must_use]
    #[inline]
    pub fn class_declaration(&self, class: Box<'a, Class<'a>>) -> Statement<'a> {
        Statement::Declaration(Declaration::ClassDeclaration(class))
    }

    #[must_use]
    #[inline]
    pub fn static_block(&self, span: Span, body: Vec<'a, Statement<'a>>) -> ClassElement<'a> {
        ClassElement::StaticBlock(self.alloc(StaticBlock { span, body }))
    }

    #[must_use]
    #[inline]
    pub fn accessor_property(
        &self,
        span: Span,
        key: PropertyKey<'a>,
        value: Option<Expression<'a>>,
        computed: bool,
        r#static: bool,
    ) -> ClassElement<'a> {
        ClassElement::AccessorProperty(self.alloc(AccessorProperty {
            span,
            key,
            value,
            computed,
            r#static,
        }))
    }

    /* ---------- Declarations ---------- */

    #[must_use]
    #[inline]
    pub fn variable_declaration(
        &self,
        span: Span,
        kind: VariableDeclarationKind,
        declarations: Vec<'a, VariableDeclarator<'a>>,
        modifiers: Modifiers<'a>,
    ) -> Box<'a, VariableDeclaration<'a>> {
        self.alloc(VariableDeclaration { span, kind, declarations, modifiers })
    }

    #[must_use]
    #[inline]
    pub fn variable_declarator(
        &self,
        span: Span,
        kind: VariableDeclarationKind,
        id: BindingPattern<'a>,
        init: Option<Expression<'a>>,
        definite: bool,
    ) -> VariableDeclarator<'a> {
        VariableDeclarator { span, kind, id, init, definite }
    }

    /* ---------- Patterns ---------- */

    #[must_use]
    #[inline]
    pub fn binding_pattern(
        &self,
        kind: BindingPatternKind<'a>,
        type_annotation: Option<Box<'a, TSTypeAnnotation<'a>>>,
        optional: bool,
    ) -> BindingPattern<'a> {
        BindingPattern { kind, type_annotation, optional }
    }

    #[must_use]
    #[inline]
    pub fn binding_identifier(&self, identifier: BindingIdentifier) -> BindingPatternKind<'a> {
        BindingPatternKind::BindingIdentifier(self.alloc(identifier))
    }

    #[must_use]
    #[inline]
    pub fn object_pattern(
        &self,
        span: Span,
        properties: Vec<'a, ObjectPatternProperty<'a>>,
    ) -> BindingPatternKind<'a> {
        BindingPatternKind::ObjectPattern(self.alloc(ObjectPattern { span, properties }))
    }

    #[must_use]
    #[inline]
    pub fn spread_element(
        &self,
        span: Span,
        argument: Expression<'a>,
    ) -> Box<'a, SpreadElement<'a>> {
        self.alloc(SpreadElement { span, argument })
    }

    #[must_use]
    #[inline]
    pub fn property(
        &self,
        span: Span,
        kind: PropertyKind,
        key: PropertyKey<'a>,
        value: PropertyValue<'a>,
        method: bool,
        shorthand: bool,
        computed: bool,
    ) -> Box<'a, Property<'a>> {
        self.alloc(Property { span, kind, key, value, method, shorthand, computed })
    }

    #[must_use]
    #[inline]
    pub fn array_pattern(
        &self,
        span: Span,
        elements: Vec<'a, Option<BindingPattern<'a>>>,
    ) -> BindingPatternKind<'a> {
        BindingPatternKind::ArrayPattern(self.alloc(ArrayPattern { span, elements }))
    }

    #[must_use]
    #[inline]
    pub fn assignment_pattern(
        &self,
        span: Span,
        left: BindingPattern<'a>,
        right: Expression<'a>,
    ) -> BindingPattern<'a> {
        let pattern = self.alloc(AssignmentPattern { span, left, right });
        BindingPattern {
            kind: BindingPatternKind::AssignmentPattern(pattern),
            type_annotation: None,
            optional: false,
        }
    }

    #[must_use]
    #[inline]
    pub fn rest_element(
        &self,
        span: Span,
        argument: BindingPattern<'a>,
    ) -> Box<'a, RestElement<'a>> {
        self.alloc(RestElement { span, argument })
    }

    #[must_use]
    #[inline]
    pub fn rest_element_pattern(&self, elem: Box<'a, RestElement<'a>>) -> BindingPattern<'a> {
        BindingPattern {
            kind: BindingPatternKind::RestElement(elem),
            type_annotation: None,
            optional: false,
        }
    }

    /* ---------- Modules ---------- */

    #[must_use]
    #[inline]
    pub fn module_declaration(&self, span: Span, kind: ModuleDeclarationKind<'a>) -> Statement<'a> {
        Statement::ModuleDeclaration(self.alloc(ModuleDeclaration { span, kind }))
    }

    #[must_use]
    #[inline]
    pub fn import_declaration(
        &self,
        specifiers: Vec<'a, ImportDeclarationSpecifier>,
        source: StringLiteral,
        assertions: Option<Vec<'a, ImportAttribute>>,
        import_kind: Option<ImportOrExportKind>,
    ) -> Box<'a, ImportDeclaration<'a>> {
        self.alloc(ImportDeclaration { specifiers, source, assertions, import_kind })
    }

    #[must_use]
    #[inline]
    pub fn export_all_declaration(
        &self,
        exported: Option<ModuleExportName>,
        source: StringLiteral,
        assertions: Option<Vec<'a, ImportAttribute>>, // Some(vec![]) for empty assertion
        export_kind: Option<ImportOrExportKind>,
    ) -> Box<'a, ExportAllDeclaration<'a>> {
        self.alloc(ExportAllDeclaration { exported, source, assertions, export_kind })
    }

    #[must_use]
    #[inline]
    pub fn export_default_declaration(
        &self,
        declaration: ExportDefaultDeclarationKind<'a>,
        exported: ModuleExportName,
    ) -> Box<'a, ExportDefaultDeclaration<'a>> {
        self.alloc(ExportDefaultDeclaration { declaration, exported })
    }

    #[must_use]
    #[inline]
    pub fn export_named_declaration(
        &self,
        declaration: Option<Declaration<'a>>,
        specifiers: Vec<'a, ExportSpecifier>,
        source: Option<StringLiteral>,
        export_kind: Option<ImportOrExportKind>, // `export type { foo }`
    ) -> Box<'a, ExportNamedDeclaration<'a>> {
        self.alloc(ExportNamedDeclaration { declaration, specifiers, source, export_kind })
    }

    /* ---------- JSX ----------------- */
    #[must_use]
    #[inline]
    pub fn jsx_element(
        &self,
        span: Span,
        opening_element: Box<'a, JSXOpeningElement<'a>>,
        closing_element: Option<Box<'a, JSXClosingElement<'a>>>,
        children: Vec<'a, JSXChild<'a>>,
    ) -> Box<'a, JSXElement<'a>> {
        self.alloc(JSXElement { span, opening_element, closing_element, children })
    }

    #[must_use]
    #[inline]
    pub fn jsx_opening_element(
        &self,
        span: Span,
        self_closing: bool,
        name: JSXElementName<'a>,
        attributes: Vec<'a, JSXAttributeItem<'a>>,
        type_parameters: Option<Box<'a, TSTypeParameterInstantiation<'a>>>,
    ) -> Box<'a, JSXOpeningElement<'a>> {
        self.alloc(JSXOpeningElement { span, self_closing, name, attributes, type_parameters })
    }

    #[must_use]
    #[inline]
    pub fn jsx_closing_element(
        &self,
        span: Span,
        name: JSXElementName<'a>,
    ) -> Box<'a, JSXClosingElement<'a>> {
        self.alloc(JSXClosingElement { span, name })
    }

    #[must_use]
    #[inline]
    pub fn jsx_fragment(
        &self,
        span: Span,
        opening_fragment: JSXOpeningFragment,
        closing_fragment: JSXClosingFragment,
        children: Vec<'a, JSXChild<'a>>,
    ) -> Box<'a, JSXFragment<'a>> {
        self.alloc(JSXFragment { span, opening_fragment, closing_fragment, children })
    }

    #[must_use]
    #[inline]
    pub fn jsx_opening_fragment(&self, span: Span) -> JSXOpeningFragment {
        JSXOpeningFragment { span }
    }

    #[must_use]
    #[inline]
    pub fn jsx_closing_fragment(&self, span: Span) -> JSXClosingFragment {
        JSXClosingFragment { span }
    }

    #[must_use]
    #[inline]
    pub fn jsx_namespaced_name(
        &self,
        span: Span,
        namespace: JSXIdentifier,
        property: JSXIdentifier,
    ) -> Box<'a, JSXNamespacedName> {
        self.alloc(JSXNamespacedName { span, namespace, property })
    }

    #[must_use]
    #[inline]
    pub fn jsx_member_expression(
        &self,
        span: Span,
        object: JSXMemberExpressionObject<'a>,
        property: JSXIdentifier,
    ) -> Box<'a, JSXMemberExpression<'a>> {
        self.alloc(JSXMemberExpression { span, object, property })
    }

    #[must_use]
    #[inline]
    pub fn jsx_expression_container(
        &self,
        span: Span,
        expression: JSXExpression<'a>,
    ) -> JSXExpressionContainer<'a> {
        JSXExpressionContainer { span, expression }
    }

    #[must_use]
    #[inline]
    pub fn jsx_spread_child(&self, span: Span, expression: Expression<'a>) -> JSXSpreadChild<'a> {
        JSXSpreadChild { span, expression }
    }

    #[must_use]
    #[inline]
    pub fn jsx_empty_expression(&self, span: Span) -> JSXEmptyExpression {
        JSXEmptyExpression { span }
    }

    #[must_use]
    #[inline]
    pub fn jsx_attribute(
        &self,
        span: Span,
        name: JSXAttributeName<'a>,
        value: Option<JSXAttributeValue<'a>>,
    ) -> Box<'a, JSXAttribute<'a>> {
        self.alloc(JSXAttribute { span, name, value })
    }

    #[must_use]
    #[inline]
    pub fn jsx_spread_attribute(
        &self,
        span: Span,
        argument: Expression<'a>,
    ) -> Box<'a, JSXSpreadAttribute<'a>> {
        self.alloc(JSXSpreadAttribute { span, argument })
    }

    #[must_use]
    #[inline]
    pub fn jsx_identifier(&self, span: Span, name: Atom) -> JSXIdentifier {
        JSXIdentifier { span, name }
    }

    #[must_use]
    #[inline]
    pub fn jsx_text(&self, span: Span, value: Atom) -> JSXText {
        JSXText { span, value }
    }

    /* ---------- TypeScript ---------- */
    #[must_use]
    #[inline]
    pub fn ts_module_declaration(
        &self,
        span: Span,
        id: TSModuleDeclarationName,
        body: TSModuleDeclarationBody<'a>,
        modifiers: Modifiers<'a>,
    ) -> Box<'a, TSModuleDeclaration<'a>> {
        self.alloc(TSModuleDeclaration { span, id, body, modifiers })
    }

    #[must_use]
    #[inline]
    pub fn ts_type_annotation(
        &self,
        span: Span,
        type_annotation: TSType<'a>,
    ) -> Box<'a, TSTypeAnnotation<'a>> {
        self.alloc(TSTypeAnnotation { span, type_annotation })
    }

    #[must_use]
    #[inline]
    pub fn ts_literal_type(&self, span: Span, literal: TSLiteral<'a>) -> TSType<'a> {
        TSType::TSLiteralType(self.alloc(TSLiteralType { span, literal }))
    }

    #[must_use]
    #[inline]
    pub fn ts_union_type(&self, span: Span, types: Vec<'a, TSType<'a>>) -> TSType<'a> {
        TSType::TSUnionType(self.alloc(TSUnionType { span, types }))
    }

    #[must_use]
    #[inline]
    pub fn ts_intersection_type(&self, span: Span, types: Vec<'a, TSType<'a>>) -> TSType<'a> {
        TSType::TSIntersectionType(self.alloc(TSIntersectionType { span, types }))
    }

    #[must_use]
    #[inline]
    pub fn ts_type_operator_type(
        &self,
        span: Span,
        operator: TSTypeOperator,
        type_annotation: TSType<'a>,
    ) -> TSType<'a> {
        TSType::TSTypeOperatorType(self.alloc(TSTypeOperatorType {
            span,
            operator,
            type_annotation,
        }))
    }

    #[must_use]
    #[inline]
    pub fn ts_array_type(&self, span: Span, element_type: TSType<'a>) -> TSType<'a> {
        TSType::TSArrayType(self.alloc(TSArrayType { span, element_type }))
    }

    #[must_use]
    #[inline]
    pub fn ts_indexed_access_type(
        &self,
        span: Span,
        object_type: TSType<'a>,
        index_type: TSType<'a>,
    ) -> TSType<'a> {
        TSType::TSIndexedAccessType(self.alloc(TSIndexedAccessType {
            span,
            object_type,
            index_type,
        }))
    }

    #[must_use]
    #[inline]
    pub fn ts_tuple_type(
        &self,
        span: Span,
        element_types: Vec<'a, TSTupleElement<'a>>,
    ) -> TSType<'a> {
        TSType::TSTupleType(self.alloc(TSTupleType { span, element_types }))
    }

    #[must_use]
    #[inline]
    pub fn ts_type_reference(
        &self,
        span: Span,
        type_name: TSTypeName<'a>,
        type_parameters: Option<Box<'a, TSTypeParameterInstantiation<'a>>>,
    ) -> TSType<'a> {
        TSType::TSTypeReference(self.alloc(TSTypeReference { span, type_name, type_parameters }))
    }

    #[must_use]
    #[inline]
    pub fn ts_type_literal(&self, span: Span, members: Vec<'a, TSSignature<'a>>) -> TSType<'a> {
        TSType::TSTypeLiteral(self.alloc(TSTypeLiteral { span, members }))
    }

    #[must_use]
    #[inline]
    pub fn ts_type_implement(
        &self,
        span: Span,
        expression: TSTypeName<'a>,
        type_parameters: Option<Box<'a, TSTypeParameterInstantiation<'a>>>,
    ) -> Box<'a, TSClassImplements<'a>> {
        self.alloc(TSClassImplements { span, expression, type_parameters })
    }

    #[must_use]
    #[inline]
    pub fn ts_type_parameter(
        &self,
        span: Span,
        name: BindingIdentifier,
        constraint: Option<TSType<'a>>,
        default: Option<TSType<'a>>,
        r#in: bool,
        out: bool,
    ) -> Box<'a, TSTypeParameter<'a>> {
        self.alloc(TSTypeParameter { span, name, constraint, default, r#in, out })
    }

    #[must_use]
    #[inline]
    pub fn ts_type_parameters(
        &self,
        span: Span,
        params: Vec<'a, Box<'a, TSTypeParameter<'a>>>,
    ) -> Box<'a, TSTypeParameterDeclaration<'a>> {
        self.alloc(TSTypeParameterDeclaration { span, params })
    }

    #[must_use]
    #[inline]
    pub fn ts_interface_heritages(
        &self,
        extends: Vec<'a, (Expression<'a>, Option<Box<'a, TSTypeParameterInstantiation<'a>>>, Span)>,
    ) -> Vec<'a, Box<'a, TSInterfaceHeritage<'a>>> {
        let mut vec = Vec::new_in(self.allocator);
        vec.extend(extends.into_iter().map(|(expression, type_parameters, span)| {
            self.alloc(TSInterfaceHeritage { span, expression, type_parameters })
        }));
        vec
    }

    #[must_use]
    #[inline]
    pub fn ts_interface_body(
        &self,
        span: Span,
        body: Vec<'a, TSSignature<'a>>,
    ) -> Box<'a, TSInterfaceBody<'a>> {
        self.alloc(TSInterfaceBody { span, body })
    }

    #[must_use]
    #[inline]
    pub fn ts_index_signature(
        &self,
        span: Span,
        parameters: Vec<'a, Box<'a, TSIndexSignatureName<'a>>>,
        type_annotation: Box<'a, TSTypeAnnotation<'a>>,
    ) -> TSSignature<'a> {
        TSSignature::TSIndexSignature(self.alloc(TSIndexSignature {
            span,
            parameters,
            type_annotation,
        }))
    }

    #[must_use]
    #[inline]
    pub fn ts_property_signature(
        &self,
        span: Span,
        computed: bool,
        optional: bool,
        readonly: bool,
        key: PropertyKey<'a>,
        type_annotation: Option<Box<'a, TSTypeAnnotation<'a>>>,
    ) -> TSSignature<'a> {
        TSSignature::TSPropertySignature(self.alloc(TSPropertySignature {
            span,
            computed,
            optional,
            readonly,
            key,
            type_annotation,
        }))
    }

    #[must_use]
    #[inline]
    pub fn ts_call_signature_declaration(
        &self,
        span: Span,
        params: Box<'a, FormalParameters<'a>>,
        return_type: Option<Box<'a, TSTypeAnnotation<'a>>>,
        type_parameters: Option<Box<'a, TSTypeParameterDeclaration<'a>>>,
    ) -> TSSignature<'a> {
        TSSignature::TSCallSignatureDeclaration(self.alloc(TSCallSignatureDeclaration {
            span,
            params,
            return_type,
            type_parameters,
        }))
    }

    #[must_use]
    #[inline]
    pub fn ts_construct_signature_declaration(
        &self,
        span: Span,
        params: Box<'a, FormalParameters<'a>>,
        return_type: Option<Box<'a, TSTypeAnnotation<'a>>>,
        type_parameters: Option<Box<'a, TSTypeParameterDeclaration<'a>>>,
    ) -> TSSignature<'a> {
        TSSignature::TSConstructSignatureDeclaration(self.alloc(TSConstructSignatureDeclaration {
            span,
            params,
            return_type,
            type_parameters,
        }))
    }

    #[must_use]
    #[inline]
    pub fn ts_method_signature(
        &self,
        span: Span,
        key: PropertyKey<'a>,
        computed: bool,
        optional: bool,
        kind: TSMethodSignatureKind,
        params: Box<'a, FormalParameters<'a>>,
        return_type: Option<Box<'a, TSTypeAnnotation<'a>>>,
        type_parameters: Option<Box<'a, TSTypeParameterDeclaration<'a>>>,
    ) -> TSSignature<'a> {
        TSSignature::TSMethodSignature(self.alloc(TSMethodSignature {
            span,
            key,
            computed,
            optional,
            kind,
            params,
            return_type,
            type_parameters,
        }))
    }

    #[must_use]
    #[inline]
    pub fn ts_module_block(
        &self,
        span: Span,
        body: Vec<'a, Statement<'a>>,
    ) -> Box<'a, TSModuleBlock<'a>> {
        self.alloc(TSModuleBlock { span, body })
    }

    #[must_use]
    #[inline]
    pub fn ts_type_arguments(
        &self,
        span: Span,
        params: Vec<'a, TSType<'a>>,
    ) -> Box<'a, TSTypeParameterInstantiation<'a>> {
        self.alloc(TSTypeParameterInstantiation { span, params })
    }

    #[must_use]
    #[inline]
    pub fn ts_non_null_expression(&self, span: Span, expression: Expression<'a>) -> Expression<'a> {
        Expression::TSNonNullExpression(self.alloc(TSNonNullExpression { span, expression }))
    }

    #[must_use]
    #[inline]
    pub fn ts_type_assertion(
        &self,
        span: Span,
        type_annotation: TSType<'a>,
        expression: Expression<'a>,
    ) -> Expression<'a> {
        Expression::TSTypeAssertion(self.alloc(TSTypeAssertion {
            span,
            type_annotation,
            expression,
        }))
    }

    #[must_use]
    #[inline]
    pub fn ts_import_equals_declaration(
        &self,
        span: Span,
        id: BindingIdentifier,
        module_reference: TSModuleReference<'a>,
        is_export: bool,
        import_kind: ImportOrExportKind,
    ) -> Declaration<'a> {
        Declaration::TSImportEqualsDeclaration(self.alloc(TSImportEqualsDeclaration {
            span,
            id,
            module_reference: self.alloc(module_reference),
            is_export,
            import_kind,
        }))
    }

    #[must_use]
    #[inline]
    pub fn ts_interface_declaration(
        &self,
        span: Span,
        id: BindingIdentifier,
        body: Box<'a, TSInterfaceBody<'a>>,
        type_parameters: Option<Box<'a, TSTypeParameterDeclaration<'a>>>,
        extends: Option<Vec<'a, Box<'a, TSInterfaceHeritage<'a>>>>,
        modifiers: Modifiers<'a>,
    ) -> Declaration<'a> {
        Declaration::TSInterfaceDeclaration(self.alloc(TSInterfaceDeclaration {
            span,
            id,
            body,
            type_parameters,
            extends,
            modifiers,
        }))
    }

    #[must_use]
    #[inline]
    pub fn ts_type_alias_declaration(
        &self,
        span: Span,
        id: BindingIdentifier,
        type_annotation: TSType<'a>,
        type_parameters: Option<Box<'a, TSTypeParameterDeclaration<'a>>>,
        modifiers: Modifiers<'a>,
    ) -> Declaration<'a> {
        Declaration::TSTypeAliasDeclaration(self.alloc(TSTypeAliasDeclaration {
            span,
            id,
            type_annotation,
            type_parameters,
            modifiers,
        }))
    }

    #[must_use]
    #[inline]
    pub fn ts_enum_declaration(
        &self,
        span: Span,
        id: BindingIdentifier,
        members: Vec<'a, TSEnumMember<'a>>,
        modifiers: Modifiers<'a>,
    ) -> Declaration<'a> {
        Declaration::TSEnumDeclaration(self.alloc(TSEnumDeclaration {
            span,
            id,
            members,
            modifiers,
        }))
    }

    #[must_use]
    #[inline]
    pub fn decorator(&self, span: Span, expression: Expression<'a>) -> Decorator<'a> {
        Decorator { span, expression }
    }

    #[must_use]
    #[inline]
    pub fn ts_void_keyword(&self, span: Span) -> TSType<'a> {
        TSType::TSVoidKeyword(self.alloc(TSVoidKeyword { span }))
    }

    #[must_use]
    #[inline]
    pub fn ts_this_keyword(&self, span: Span) -> TSType<'a> {
        TSType::TSThisKeyword(self.alloc(TSThisKeyword { span }))
    }

    #[must_use]
    #[inline]
    pub fn ts_any_keyword(&self, span: Span) -> TSType<'a> {
        TSType::TSAnyKeyword(self.alloc(TSAnyKeyword { span }))
    }

    #[must_use]
    #[inline]
    pub fn ts_unknown_keyword(&self, span: Span) -> TSType<'a> {
        TSType::TSUnknownKeyword(self.alloc(TSUnknownKeyword { span }))
    }

    #[must_use]
    #[inline]
    pub fn ts_number_keyword(&self, span: Span) -> TSType<'a> {
        TSType::TSNumberKeyword(self.alloc(TSNumberKeyword { span }))
    }

    #[must_use]
    #[inline]
    pub fn ts_boolean_keyword(&self, span: Span) -> TSType<'a> {
        TSType::TSBooleanKeyword(self.alloc(TSBooleanKeyword { span }))
    }

    #[must_use]
    #[inline]
    pub fn ts_object_keyword(&self, span: Span) -> TSType<'a> {
        TSType::TSObjectKeyword(self.alloc(TSObjectKeyword { span }))
    }

    #[must_use]
    #[inline]
    pub fn ts_string_keyword(&self, span: Span) -> TSType<'a> {
        TSType::TSStringKeyword(self.alloc(TSStringKeyword { span }))
    }

    #[must_use]
    #[inline]
    pub fn ts_bigint_keyword(&self, span: Span) -> TSType<'a> {
        TSType::TSBigIntKeyword(self.alloc(TSBigIntKeyword { span }))
    }

    #[must_use]
    #[inline]
    pub fn ts_symbol_keyword(&self, span: Span) -> TSType<'a> {
        TSType::TSSymbolKeyword(self.alloc(TSSymbolKeyword { span }))
    }

    #[must_use]
    #[inline]
    pub fn ts_null_keyword(&self, span: Span) -> TSType<'a> {
        TSType::TSNullKeyword(self.alloc(TSNullKeyword { span }))
    }

    #[must_use]
    #[inline]
    pub fn ts_undefined_keyword(&self, span: Span) -> TSType<'a> {
        TSType::TSUndefinedKeyword(self.alloc(TSUndefinedKeyword { span }))
    }

    #[must_use]
    #[inline]
    pub fn ts_never_keyword(&self, span: Span) -> TSType<'a> {
        TSType::TSNeverKeyword(self.alloc(TSNeverKeyword { span }))
    }

    #[must_use]
    #[inline]
    pub fn ts_template_literal_type(
        &self,
        span: Span,
        quasis: Vec<'a, TemplateElement>,
        types: Vec<'a, TSType<'a>>,
    ) -> TSType<'a> {
        TSType::TSTemplateLiteralType(self.alloc(TSTemplateLiteralType { span, quasis, types }))
    }

    #[must_use]
    #[inline]
    pub fn ts_type_query_type(
        &self,
        span: Span,
        expr_name: TSTypeName<'a>,
        type_parameters: Option<Box<'a, TSTypeParameterInstantiation<'a>>>,
    ) -> TSType<'a> {
        TSType::TSTypeQuery(self.alloc(TSTypeQuery { span, expr_name, type_parameters }))
    }

    #[must_use]
    #[inline]
    pub fn ts_conditional_type(
        &self,
        span: Span,
        check_type: TSType<'a>,
        extends_type: TSType<'a>,
        true_type: TSType<'a>,
        false_type: TSType<'a>,
    ) -> TSType<'a> {
        TSType::TSConditionalType(self.alloc(TSConditionalType {
            span,
            check_type,
            extends_type,
            true_type,
            false_type,
        }))
    }

    #[must_use]
    #[inline]
    pub fn ts_mapped_type(
        &self,
        span: Span,
        type_parameter: Box<'a, TSTypeParameter<'a>>,
        name_type: Option<TSType<'a>>,
        type_annotation: TSType<'a>,
        optional: TSMappedTypeModifierOperator,
        readonly: TSMappedTypeModifierOperator,
    ) -> TSType<'a> {
        TSType::TSMappedType(self.alloc(TSMappedType {
            span,
            type_parameter,
            name_type,
            type_annotation,
            optional,
            readonly,
        }))
    }

    #[must_use]
    #[inline]
    pub fn ts_import_type(
        &self,
        span: Span,
        is_type_of: bool,
        parameter: TSType<'a>,
        qualifier: Option<TSTypeName<'a>>,
        type_parameters: Option<Box<'a, TSTypeParameterInstantiation<'a>>>,
    ) -> TSType<'a> {
        TSType::TSImportType(self.alloc(TSImportType {
            span,
            is_type_of,
            parameter,
            qualifier,
            type_parameters,
        }))
    }

    #[must_use]
    #[inline]
    pub fn ts_constructor_type(
        &self,
        span: Span,
        r#abstract: bool,
        params: Box<'a, FormalParameters<'a>>,
        return_type: Box<'a, TSTypeAnnotation<'a>>,
        type_parameters: Option<Box<'a, TSTypeParameterDeclaration<'a>>>,
    ) -> TSType<'a> {
        TSType::TSConstructorType(self.alloc(TSConstructorType {
            span,
            r#abstract,
            params,
            return_type,
            type_parameters,
        }))
    }

    #[must_use]
    #[inline]
    pub fn ts_function_type(
        &self,
        span: Span,
        params: Box<'a, FormalParameters<'a>>,
        return_type: Box<'a, TSTypeAnnotation<'a>>,
        type_parameters: Option<Box<'a, TSTypeParameterDeclaration<'a>>>,
    ) -> TSType<'a> {
        TSType::TSFunctionType(self.alloc(TSFunctionType {
            span,
            params,
            return_type,
            type_parameters,
        }))
    }

    #[must_use]
    #[inline]
    pub fn ts_infer_type(
        &self,
        span: Span,
        type_parameter: Box<'a, TSTypeParameter<'a>>,
    ) -> TSType<'a> {
        TSType::TSInferType(self.alloc(TSInferType { span, type_parameter }))
    }

    #[must_use]
    #[inline]
    pub fn ts_type_predicate(
        &self,
        span: Span,
        parameter_name: TSTypePredicateName,
        asserts: bool,
        type_annotation: Option<Box<'a, TSTypeAnnotation<'a>>>,
    ) -> TSType<'a> {
        TSType::TSTypePredicate(self.alloc(TSTypePredicate {
            span,
            parameter_name,
            asserts,
            type_annotation,
        }))
    }

    /* JSDoc */
    #[must_use]
    #[inline]
    pub fn js_doc_nullable_type(
        &self,
        span: Span,
        type_annotation: TSType<'a>,
        postfix: bool,
    ) -> TSType<'a> {
        TSType::JSDocNullableType(self.alloc(JSDocNullableType { span, type_annotation, postfix }))
    }

    #[must_use]
    #[inline]
    pub fn js_doc_unknown_type(&self, span: Span) -> TSType<'a> {
        TSType::JSDocUnknownType(self.alloc(JSDocUnknownType { span }))
    }
}
