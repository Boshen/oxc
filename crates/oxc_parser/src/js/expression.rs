use oxc_allocator::Box;
use oxc_ast::{ast::*, Atom, Span};
use oxc_diagnostics::Result;

use super::function::IsParenthesizedArrowFunction;
use super::grammar::CoverGrammar;
use super::list::{ArrayExpressionList, CallArguments, SequenceExpressionList};
use super::operator::{
    map_assignment_operator, map_binary_operator, map_logical_operator, map_unary_operator,
    map_update_operator, BindingPower,
};
use crate::{
    diagnostics,
    lexer::{Kind, TokenValue},
    list::SeparatedList,
    Parser,
};

impl<'a> Parser<'a> {
    pub fn parse_paren_expression(&mut self) -> Result<Expression<'a>> {
        self.expect(Kind::LParen)?;
        let expression = self.parse_expression()?;
        self.expect(Kind::RParen)?;
        Ok(expression)
    }

    /// Section Expression `https://tc39.es/ecma262/#sec-ecmascript-language-expressions`
    pub fn parse_expression(&mut self) -> Result<Expression<'a>> {
        let span = self.start_span();

        // clear the decorator context when parsing Expression, as it should be unambiguous when parsing a decorator
        let save_decorator_context = self.ctx.has_decorator();
        if save_decorator_context {
            self.ctx = self.ctx.and_decorator(false);
        }

        let lhs = self.parse_assignment_expression_base()?;
        if !self.at(Kind::Comma) {
            return Ok(lhs);
        }

        let expr = self.parse_sequence_expression(span, lhs)?;

        if save_decorator_context {
            self.ctx = self.ctx.and_decorator(true);
        }

        Ok(expr)
    }

    /// `PrimaryExpression`: Identifier Reference
    pub fn parse_identifier_expression(&mut self) -> Result<Expression<'a>> {
        let ident = self.parse_identifier_reference()?;
        Ok(self.ast.identifier_expression(ident))
    }

    pub fn parse_identifier_reference(&mut self) -> Result<IdentifierReference> {
        // allow `await` and `yield`, let semantic analysis report error
        if !self.cur_kind().is_identifier_reference(false, false) {
            return self.unexpected();
        }
        let (span, name) = self.parse_identifier_kind(Kind::Ident);
        Ok(IdentifierReference { span, name })
    }

    /// `BindingIdentifier` : Identifier
    pub fn parse_binding_identifier(&mut self) -> Result<BindingIdentifier> {
        if !self.cur_kind().is_binding_identifier() {
            return self.unexpected();
        }
        let (span, name) = self.parse_identifier_kind(Kind::Ident);
        Ok(BindingIdentifier { span, name })
    }

    pub fn parse_label_identifier(&mut self) -> Result<LabelIdentifier> {
        if !self.cur_kind().is_label_identifier(self.ctx.has_yield(), self.ctx.has_await()) {
            return self.unexpected();
        }
        let (span, name) = self.parse_identifier_kind(Kind::Ident);
        Ok(LabelIdentifier { span, name })
    }

    pub fn parse_identifier_name(&mut self) -> Result<IdentifierName> {
        if !self.cur_kind().is_identifier_name() {
            return self.unexpected();
        }
        let (span, name) = self.parse_identifier_kind(Kind::Ident);
        Ok(IdentifierName { span, name })
    }

    /// Parse keyword kind as identifier
    pub fn parse_keyword_identifier(&mut self, kind: Kind) -> IdentifierName {
        let (span, name) = self.parse_identifier_kind(kind);
        IdentifierName { span, name }
    }

    pub fn parse_identifier_kind(&mut self, kind: Kind) -> (Span, Atom) {
        let span = self.start_span();
        let name = match std::mem::take(&mut self.token.value) {
            TokenValue::String(value) => value,
            _ => "".into(),
        };
        self.bump_remap(kind);
        (self.end_span(span), name)
    }

    /// Section `https://tc39.es/ecma262/#prod-PrivateIdentifier`
    /// `PrivateIdentifier` ::
    ///     # `IdentifierName`
    /// # Panics
    pub fn parse_private_identifier(&mut self) -> PrivateIdentifier {
        let span = self.start_span();
        let name = self.cur_atom().unwrap().clone();
        self.bump_any();
        PrivateIdentifier { span: self.end_span(span), name }
    }

    /// Section Primary Expression `https://tc39.es/ecma262/#sec-primary-expression`
    /// `PrimaryExpression`[Yield, Await] :
    ///     this
    ///     `IdentifierReference`[?Yield, ?Await]
    ///     Literal
    ///     `ArrayLiteral`[?Yield, ?Await]
    ///     `ObjectLiteral`[?Yield, ?Await]
    ///     `FunctionExpression`
    ///     `ClassExpression`[?Yield, ?Await]
    ///     `GeneratorExpression`
    ///     `AsyncFunctionExpression`
    ///     `AsyncGeneratorExpression`
    ///     `RegularExpressionLiteral`
    ///     `TemplateLiteral`[?Yield, ?Await, ~Tagged]
    ///     `CoverParenthesizedExpressionAndArrowParameterList`[?Yield, ?Await]
    fn parse_primary_expression(&mut self) -> Result<Expression<'a>> {
        let span = self.start_span();

        // AsyncFunctionExpression
        // AsyncGeneratorExpression
        if self.at_function_with_async() {
            let r#async = self.eat(Kind::Async);
            return self.parse_function_expression(span, r#async);
        }

        match &self.cur_kind() {
            Kind::Ident => self.parse_identifier_expression(), // fast path, keywords are checked at the end
            // Literal, RegularExpressionLiteral
            kind if kind.is_literal() => self.parse_literal_expression(),
            // ArrayLiteral
            Kind::LBrack => self.parse_array_expression(),
            // ObjectLiteral
            Kind::LCurly => self.parse_object_expression(),
            // FunctionExpession, GeneratorExpression
            Kind::Function => self.parse_function_expression(span, false),
            // ClassExpression
            Kind::Class => self.parse_class_expression(),
            // This
            Kind::This => Ok(self.parse_this_expression()),
            // TemplateLiteral
            Kind::NoSubstitutionTemplate | Kind::TemplateHead => {
                self.parse_template_literal_expression(false)
            }
            Kind::New => self.parse_new_expression(),
            Kind::Super => Ok(self.parse_super()),
            Kind::Import => {
                let span = self.start_span();
                let identifier = self.parse_keyword_identifier(Kind::Import);
                match self.cur_kind() {
                    Kind::Dot => self.parse_meta_property(span, identifier),
                    Kind::LParen => self.parse_import_expression(span),
                    _ => self.unexpected(),
                }
            }
            Kind::LParen => self.parse_parenthesized_expression(span),
            Kind::Slash | Kind::SlashEq => {
                self.read_regex();
                self.parse_literal_regexp()
                    .map(|literal| self.ast.literal_regexp_expression(literal))
            }
            // JSXElement, JSXFragment
            Kind::LAngle if self.source_type.is_jsx() => self.parse_jsx_expression(),
            _ => self.parse_identifier_expression(),
        }
    }

    fn parse_parenthesized_expression(&mut self, span: Span) -> Result<Expression<'a>> {
        let has_in = self.ctx.has_in();
        self.ctx = self.ctx.and_in(true);
        let list = SequenceExpressionList::parse(self)?;
        self.ctx = self.ctx.and_in(has_in);

        let mut expressions = list.elements;

        // ParenthesizedExpression is from acorn --preserveParens
        let expression = if expressions.len() == 1 {
            expressions.remove(0)
        } else {
            if expressions.is_empty() {
                self.error(diagnostics::EmptyParenthesizedExpression(list.span));
            }
            self.ast.sequence_expression(list.span, expressions)
        };

        Ok(self.ast.parenthesized_expression(self.end_span(span), expression))
    }

    /// Section 13.2.2 This Expression
    fn parse_this_expression(&mut self) -> Expression<'a> {
        let span = self.start_span();
        self.bump_any();
        self.ast.this_expression(self.end_span(span))
    }

    /// [Literal Expression](https://tc39.es/ecma262/#prod-Literal)
    /// parses string | true | false | null | number
    pub fn parse_literal_expression(&mut self) -> Result<Expression<'a>> {
        match self.cur_kind() {
            Kind::Str => self
                .parse_literal_string()
                .map(|literal| self.ast.literal_string_expression(literal)),
            Kind::True | Kind::False => self
                .parse_literal_boolean()
                .map(|literal| self.ast.literal_boolean_expression(literal)),
            Kind::Null => {
                let literal = self.parse_literal_null();
                Ok(self.ast.literal_null_expression(literal))
            }
            Kind::RegExp => self
                .parse_literal_regexp()
                .map(|literal| self.ast.literal_regexp_expression(literal)),
            kind if kind.is_number() => {
                if self.cur_src().ends_with('n') {
                    self.parse_literal_bigint()
                        .map(|literal| self.ast.literal_bigint_expression(literal))
                } else {
                    self.parse_literal_number()
                        .map(|literal| self.ast.literal_number_expression(literal))
                }
            }
            _ => self.unexpected(),
        }
    }

    pub fn parse_literal_boolean(&mut self) -> Result<BooleanLiteral> {
        let span = self.start_span();
        let value = match self.cur_kind() {
            Kind::True => true,
            Kind::False => false,
            _ => return self.unexpected(),
        };
        self.bump_any();
        Ok(BooleanLiteral { span: self.end_span(span), value })
    }

    pub fn parse_literal_null(&mut self) -> NullLiteral {
        let span = self.start_span();
        self.bump_any(); // bump `null`
        NullLiteral { span: self.end_span(span) }
    }

    pub fn parse_literal_number(&mut self) -> Result<NumberLiteral<'a>> {
        let span = self.start_span();
        let base = match self.cur_kind() {
            Kind::Float | Kind::Decimal => NumberBase::Decimal,
            Kind::Binary => NumberBase::Binary,
            Kind::Octal => NumberBase::Octal,
            Kind::Hex => NumberBase::Hex,
            _ => return self.unexpected(),
        };
        let value = self.cur_token().value.as_number();
        let raw = self.cur_src();
        self.bump_any();
        Ok(NumberLiteral::new(self.end_span(span), value, raw, base))
    }

    pub fn parse_literal_bigint(&mut self) -> Result<BigintLiteral> {
        let span = self.start_span();
        let value = match self.cur_kind() {
            kind if kind.is_number() => self.cur_token().value.as_bigint(),
            _ => return self.unexpected(),
        };
        self.bump_any();
        Ok(BigintLiteral { span: self.end_span(span), value })
    }

    pub fn parse_literal_regexp(&mut self) -> Result<RegExpLiteral> {
        let span = self.start_span();
        let r = match self.cur_kind() {
            Kind::RegExp => self.cur_token().value.as_regex(),
            _ => return self.unexpected(),
        };
        self.bump_any();
        Ok(RegExpLiteral {
            span: self.end_span(span),
            value: EmptyObject {},
            regex: RegExp { pattern: r.pattern, flags: r.flags },
        })
    }

    pub fn parse_literal_string(&mut self) -> Result<StringLiteral> {
        if !self.at(Kind::Str) {
            return self.unexpected();
        }
        let TokenValue::String(value) = std::mem::take(&mut self.token.value) else {
            unreachable!()
        };
        let span = self.start_span();
        self.bump_any();
        Ok(StringLiteral { span: self.end_span(span), value })
    }

    /// Section Array Expression `https://tc39.es/ecma262/#prod-ArrayLiteral`
    /// `ArrayLiteral`[Yield, Await]:
    ///     [ Elision opt ]
    ///     [ `ElementList`[?Yield, ?Await] ]
    ///     [ `ElementList`[?Yield, ?Await] , Elisionopt ]
    pub fn parse_array_expression(&mut self) -> Result<Expression<'a>> {
        let span = self.start_span();
        let has_in = self.ctx.has_in();
        self.ctx = self.ctx.and_in(true);
        let list = ArrayExpressionList::parse(self)?;
        self.ctx = self.ctx.and_in(has_in);
        Ok(self.ast.array_expression(self.end_span(span), list.elements, list.trailing_comma))
    }

    /// Section Template Literal `https://tc39.es/ecma262/#prod-TemplateLiteral`
    /// `TemplateLiteral`[Yield, Await, Tagged] :
    ///     `NoSubstitutionTemplate`
    ///     `SubstitutionTemplate`[?Yield, ?Await, ?Tagged]
    fn parse_template_literal(&mut self, tagged: bool) -> Result<TemplateLiteral<'a>> {
        let span = self.start_span();
        let mut expressions = self.ast.new_vec();
        let mut quasis = self.ast.new_vec();
        match self.cur_kind() {
            Kind::NoSubstitutionTemplate => {
                quasis.push(self.parse_template_element(tagged));
            }
            Kind::TemplateHead => {
                quasis.push(self.parse_template_element(tagged));
                expressions.push(self.parse_expression()?);
                self.re_lex_template_substitution_tail();
                loop {
                    match self.cur_kind() {
                        Kind::Eof => self.expect(Kind::TemplateTail)?,
                        Kind::TemplateTail => {
                            quasis.push(self.parse_template_element(tagged));
                            break;
                        }
                        Kind::TemplateMiddle => {
                            quasis.push(self.parse_template_element(tagged));
                        }
                        _ => {
                            expressions.push(self.parse_expression()?);
                            self.re_lex_template_substitution_tail();
                        }
                    }
                }
            }
            _ => unreachable!("parse_template_literal"),
        }
        Ok(TemplateLiteral { span: self.end_span(span), quasis, expressions })
    }

    fn parse_template_literal_expression(&mut self, tagged: bool) -> Result<Expression<'a>> {
        self.parse_template_literal(tagged)
            .map(|template_literal| self.ast.template_literal_expression(template_literal))
    }

    fn parse_tagged_template(
        &mut self,
        span: Span,
        lhs: Expression<'a>,
        in_optional_chain: bool,
        type_parameters: Option<Box<'a, TSTypeParameterInstantiation<'a>>>,
    ) -> Result<Expression<'a>> {
        let quasi = self.parse_template_literal(true)?;
        let span = self.end_span(span);
        // OptionalChain :
        //   ?. TemplateLiteral
        //   OptionalChain TemplateLiteral
        // It is a Syntax Error if any source text is matched by this production.
        // https://tc39.es/ecma262/#sec-left-hand-side-expressions-static-semantics-early-errors
        if in_optional_chain {
            self.error(diagnostics::OptionalChainTaggedTemplate(quasi.span));
        }
        Ok(self.ast.tagged_template_expression(span, lhs, quasi, type_parameters))
    }

    pub fn parse_template_element(&mut self, tagged: bool) -> TemplateElement {
        let span = self.start_span();
        let cur_kind = self.cur_kind();
        let end_offset: u32 = match cur_kind {
            Kind::TemplateHead | Kind::TemplateMiddle => 2,
            Kind::NoSubstitutionTemplate | Kind::TemplateTail => 1,
            _ => unreachable!(),
        };

        // cooked = None when template literal has invalid escape sequence
        let cooked = self.cur_atom().map(Clone::clone);

        let raw = &self.cur_src()[1..self.cur_src().len() - end_offset as usize];
        let raw = Atom::from(if cooked.is_some() && raw.contains('\r') {
            self.ast.new_str(raw.replace("\r\n", "\n").replace('\r', "\n").as_str())
        } else {
            raw
        });

        self.bump_any();

        let mut span = self.end_span(span);
        span.start += 1;
        span.end -= end_offset;

        if !tagged && cooked.is_none() {
            self.error(diagnostics::TemplateLiteral(span));
        }

        let tail = matches!(cur_kind, Kind::TemplateTail | Kind::NoSubstitutionTemplate);
        TemplateElement { span, tail, value: TemplateElementValue { raw, cooked } }
    }

    /// Section 13.3 Meta Property
    fn parse_meta_property(&mut self, span: Span, meta: IdentifierName) -> Result<Expression<'a>> {
        self.bump_any(); // bump `.`
        let property = match self.cur_kind() {
            Kind::Meta => self.parse_keyword_identifier(Kind::Meta),
            Kind::Target => self.parse_keyword_identifier(Kind::Target),
            _ => self.parse_identifier_name()?,
        };
        let span = self.end_span(span);
        Ok(self.ast.meta_property(span, meta, property))
    }

    /// Section 13.3 Left-Hand-Side Expression
    pub fn parse_lhs_expression(&mut self) -> Result<Expression<'a>> {
        let span = self.start_span();
        let mut in_optional_chain = false;
        let lhs = self.parse_member_expression_base(&mut in_optional_chain)?;
        let lhs = self.parse_call_expression(span, lhs, &mut in_optional_chain)?;
        if in_optional_chain {
            let span = self.end_span(span);
            Ok(self.map_to_chain_expression(span, lhs))
        } else {
            Ok(lhs)
        }
    }

    fn map_to_chain_expression(&mut self, span: Span, expr: Expression<'a>) -> Expression<'a> {
        match expr {
            Expression::MemberExpression(result) => {
                self.ast.chain_expression(span, ChainElement::MemberExpression(result))
            }
            Expression::CallExpression(result) => {
                self.ast.chain_expression(span, ChainElement::CallExpression(result))
            }
            expr => expr,
        }
    }

    /// Section 13.3 Member Expression
    fn parse_member_expression_base(
        &mut self,
        in_optional_chain: &mut bool,
    ) -> Result<Expression<'a>> {
        let span = self.start_span();
        self.parse_primary_expression()
            .and_then(|lhs| self.parse_member_expression_rhs(span, lhs, in_optional_chain))
    }

    /// Section 13.3 Super Call
    fn parse_super(&mut self) -> Expression<'a> {
        let span = self.start_span();
        self.bump_any(); // bump `super`
        let span = self.end_span(span);

        // The `super` keyword can appear at below:
        // SuperProperty:
        //     super [ Expression ]
        //     super . IdentifierName
        // SuperCall:
        //     super ( Arguments )
        if !matches!(self.cur_kind(), Kind::Dot | Kind::LBrack | Kind::LParen) {
            self.error(diagnostics::UnexpectedSuper(span));
        }

        self.ast.super_(span)
    }

    /// parse rhs of a member expression, starting from lhs
    fn parse_member_expression_rhs(
        &mut self,
        lhs_span: Span,
        lhs: Expression<'a>,
        in_optional_chain: &mut bool,
    ) -> Result<Expression<'a>> {
        let mut lhs = lhs;
        while !self.at(Kind::Eof) {
            lhs = match self.cur_kind() {
                // Decorator context does not parse computed member expressions, e.g.
                // `class C { @dec() ["method"]() {} }`
                Kind::LBrack if !self.ctx.has_decorator() => {
                    self.parse_computed_member_expression(lhs_span, lhs, false)?
                }
                Kind::Dot => self.parse_static_member_expression(lhs_span, lhs, false)?,
                Kind::QuestionDot => {
                    *in_optional_chain = true;
                    match self.peek_kind() {
                        Kind::LBrack => {
                            self.bump_any(); // bump `?.`
                            self.parse_computed_member_expression(lhs_span, lhs, true)?
                        }
                        Kind::PrivateIdentifier => {
                            self.parse_static_member_expression(lhs_span, lhs, true)?
                        }
                        kind if kind.is_identifier_name() => {
                            self.parse_static_member_expression(lhs_span, lhs, true)?
                        }
                        _ => break,
                    }
                }
                Kind::Bang if !self.cur_token().is_on_new_line && self.ts_enabled() => {
                    self.bump_any();
                    self.ast.ts_non_null_expression(self.end_span(lhs_span), lhs)
                }
                Kind::LAngle | Kind::ShiftLeft if self.ts_enabled() => {
                    if let Some(arguments) = self.parse_ts_type_arguments_in_expression()? {
                        lhs = Expression::TSInstantiationExpression(self.ast.alloc(
                            TSInstantiationExpression {
                                span: self.end_span(lhs_span),
                                expression: lhs,
                                type_parameters: arguments,
                            },
                        ));
                        continue;
                    }
                    break;
                }
                _ => break,
            };
        }
        Ok(lhs)
    }

    /// Section 13.3 `MemberExpression`
    /// static member `a.b`
    fn parse_static_member_expression(
        &mut self,
        lhs_span: Span,
        lhs: Expression<'a>,
        optional: bool,
    ) -> Result<Expression<'a>> {
        self.bump_any(); // advance `.` or `?.`
        if self.cur_kind() == Kind::PrivateIdentifier {
            let private_ident = self.parse_private_identifier();
            Ok(self.ast.private_field_expression(
                self.end_span(lhs_span),
                lhs,
                private_ident,
                optional,
            ))
        } else {
            let ident = self.parse_identifier_name()?;
            Ok(self.ast.static_member_expression(self.end_span(lhs_span), lhs, ident, optional))
        }
    }

    /// Section 13.3 `MemberExpression`
    /// `MemberExpression`[Yield, Await] :
    ///   `MemberExpression`[?Yield, ?Await] [ Expression[+In, ?Yield, ?Await] ]
    fn parse_computed_member_expression(
        &mut self,
        lhs_span: Span,
        lhs: Expression<'a>,
        optional: bool,
    ) -> Result<Expression<'a>> {
        self.bump_any(); // advance `[`
        let has_in = self.ctx.has_in();
        self.ctx = self.ctx.and_in(true);
        let property = self.parse_expression()?;
        self.ctx = self.ctx.and_in(has_in);
        self.expect(Kind::RBrack)?;
        Ok(self.ast.computed_member_expression(self.end_span(lhs_span), lhs, property, optional))
    }

    /// `NewExpression` : new `NewExpression`
    /// `https://tc39.es/ecma262/#sec-new-operator`
    fn parse_new_expression(&mut self) -> Result<Expression<'a>> {
        let span = self.start_span();
        let identifier = self.parse_keyword_identifier(Kind::New);
        if self.at(Kind::Dot) {
            return self.parse_meta_property(span, identifier);
        }
        let rhs_span = self.start_span();

        let mut optional = false;
        let mut callee = self.parse_member_expression_base(&mut optional)?;

        let mut type_parameter = None;
        if let Expression::TSInstantiationExpression(instantiation_expr) = callee {
            let instantiation_expr = Box::into_inner(instantiation_expr);
            type_parameter.replace(instantiation_expr.type_parameters);
            callee = instantiation_expr.expression;
        }

        // parse `new ident` without arguments
        let arguments = if self.at(Kind::LParen) {
            CallArguments::parse(self)?.elements
        } else {
            self.ast.new_vec()
        };

        if matches!(callee, Expression::ImportExpression(_)) {
            self.error(diagnostics::NewDynamicImport(self.end_span(rhs_span)));
        }

        let span = self.end_span(span);

        if optional {
            self.error(diagnostics::NewOptionalChain(span));
        }

        Ok(self.ast.new_expression(span, callee, arguments, type_parameter))
    }

    /// Section 13.3 Call Expression
    fn parse_call_expression(
        &mut self,
        lhs_span: Span,
        lhs: Expression<'a>,
        in_optional_chain: &mut bool,
    ) -> Result<Expression<'a>> {
        let mut lhs = lhs;
        let mut type_arguments = None;
        loop {
            lhs = self.parse_member_expression_rhs(lhs_span, lhs, in_optional_chain)?;
            let optional_call = self.eat(Kind::QuestionDot);
            *in_optional_chain = if optional_call { true } else { *in_optional_chain };
            if let Expression::TSInstantiationExpression(expr) = lhs {
                let expr = Box::into_inner(expr);
                type_arguments.replace(expr.type_parameters);
                lhs = expr.expression;
            }
            match self.cur_kind() {
                Kind::LParen => {
                    lhs = self.parse_call_arguments(
                        lhs_span,
                        lhs,
                        optional_call,
                        type_arguments.take(),
                    )?;
                }
                Kind::NoSubstitutionTemplate | Kind::TemplateHead => {
                    lhs = self.parse_tagged_template(
                        lhs_span,
                        lhs,
                        *in_optional_chain,
                        type_arguments.take(),
                    )?;
                }
                Kind::LAngle | Kind::ShiftLeft if self.ts_enabled() => {
                    let result = self.try_parse(|p| {
                        let arguments = p.parse_ts_type_arguments()?;
                        if p.at(Kind::RAngle) {
                            // a<b>>c is not (a<b>)>c, but a<(b>>c)
                            return p.unexpected();
                        }

                        // a<b>c is (a<b)>c
                        if !p.at(Kind::LParen)
                            && !p.at(Kind::NoSubstitutionTemplate)
                            && !p.at(Kind::TemplateHead)
                            && p.cur_kind().is_at_expression()
                            && !p.cur_token().is_on_new_line
                        {
                            return p.unexpected();
                        }

                        type_arguments = arguments;
                        Ok(())
                    });
                    if result.is_err() {
                        break;
                    }
                }
                _ => break,
            }
        }

        Ok(lhs)
    }

    fn parse_call_arguments(
        &mut self,
        lhs_span: Span,
        lhs: Expression<'a>,
        optional: bool,
        type_parameters: Option<Box<'a, TSTypeParameterInstantiation<'a>>>,
    ) -> Result<Expression<'a>> {
        let has_in = self.ctx.has_in();
        self.ctx = self.ctx.and_in(true);
        let call_arguments = CallArguments::parse(self)?;
        self.ctx = self.ctx.and_in(has_in);
        Ok(self.ast.call_expression(
            self.end_span(lhs_span),
            lhs,
            call_arguments.elements,
            optional,
            type_parameters,
        ))
    }

    /// Section 13.4 Update Expression
    fn parse_update_expression(&mut self) -> Result<Expression<'a>> {
        let span = self.start_span();
        let lhs = self.parse_lhs_expression()?;
        // ++ -- postfix update expressions
        if self.cur_kind().is_update_operator() && !self.cur_token().is_on_new_line {
            let operator = map_update_operator(self.cur_kind());
            self.bump_any();
            let lhs = SimpleAssignmentTarget::cover(lhs, self)?;
            return Ok(self.ast.update_expression(self.end_span(span), operator, false, lhs));
        }
        Ok(lhs)
    }

    /// Section 13.5 Unary Expression
    pub fn parse_unary_expression_base(&mut self, lhs_span: Span) -> Result<Expression<'a>> {
        // [+Await] AwaitExpression
        if self.is_await_expression() {
            return self.parse_await_expression(lhs_span);
        }

        if (self.at(Kind::LAngle) || self.at(Kind::ShiftLeft))
            && !self.source_type.is_jsx()
            && self.ts_enabled()
        {
            return self.parse_ts_type_assertion();
        }

        // ++ -- prefix update expressions
        if self.cur_kind().is_update_operator() {
            let operator = map_update_operator(self.cur_kind());
            self.bump_any();
            let argument = self.parse_unary_expression_base(lhs_span)?;
            let argument = SimpleAssignmentTarget::cover(argument, self)?;
            return Ok(self.ast.update_expression(
                self.end_span(lhs_span),
                operator,
                true,
                argument,
            ));
        }

        // delete void typeof + - ~ ! prefix unary expressions
        if self.cur_kind().is_unary_operator() {
            return self.parse_unary_expression();
        }

        self.parse_update_expression()
    }

    fn parse_unary_expression(&mut self) -> Result<Expression<'a>> {
        let span = self.start_span();
        let operator = map_unary_operator(self.cur_kind());
        self.bump_any();
        let argument = self.parse_unary_expression_base(span)?;
        Ok(self.ast.unary_expression(self.end_span(span), operator, true, argument))
    }

    fn parse_binary_or_logical_expression_base(
        &mut self,
        lhs_binding_power: BindingPower,
    ) -> Result<Expression<'a>> {
        let lhs_span = self.start_span();

        let lhs = if self.ctx.has_in() && self.at(Kind::PrivateIdentifier) {
            let left = self.parse_private_identifier();
            self.expect(Kind::In)?;
            let right = self.parse_unary_expression_base(lhs_span)?;
            Expression::PrivateInExpression(self.ast.alloc(PrivateInExpression {
                span: self.end_span(lhs_span),
                left,
                operator: BinaryOperator::In,
                right,
            }))
        } else {
            self.parse_unary_expression_base(lhs_span)?
        };

        self.parse_binary_or_logical_expression_recursive(lhs_span, lhs, lhs_binding_power)
    }

    /// Section 13.6 - 13.13 Binary Expression
    fn parse_binary_or_logical_expression_recursive(
        &mut self,
        lhs_span: Span,
        lhs: Expression<'a>,
        min_binding_power: BindingPower,
    ) -> Result<Expression<'a>> {
        // Pratt Parsing Algorithm
        // https://matklad.github.io/2020/04/13/simple-but-powerful-pratt-parsing.html
        let mut lhs = lhs;
        loop {
            // re-lex for `>=` `>>` `>>>`
            // This is need for jsx `<div>=</div>` case
            let kind = self.re_lex_right_angle();

            // Omit the In keyword for the grammer in 13.10 Relational Operators
            // RelationalExpression[In, Yield, Await] :
            // [+In] RelationalExpression[+In, ?Yield, ?Await] in ShiftExpression[?Yield, ?Await]
            if kind == Kind::In && !self.ctx.has_in()
                || (kind == Kind::As && self.cur_token().is_on_new_line)
            {
                break;
            }

            let Some(left_binding_power) = BindingPower::value(kind) else { break };

            let stop = if BindingPower::is_right_associative(left_binding_power) {
                left_binding_power < min_binding_power
            } else {
                left_binding_power <= min_binding_power
            };

            if stop {
                break;
            }

            self.bump_any(); // bump operator

            if self.ts_enabled() && kind == Kind::As {
                let type_annotation = self.parse_ts_type()?;
                lhs = Expression::TSAsExpression(self.ast.alloc(TSAsExpression {
                    span: self.end_span(lhs_span),
                    expression: lhs,
                    type_annotation,
                }));
                continue;
            }

            let rhs = self.parse_binary_or_logical_expression_base(left_binding_power)?;

            lhs = if kind.is_logical_operator() {
                self.ast.logical_expression(
                    self.end_span(lhs_span),
                    lhs,
                    map_logical_operator(kind),
                    rhs,
                )
            } else if kind.is_binary_operator() {
                self.ast.binary_expression(
                    self.end_span(lhs_span),
                    lhs,
                    map_binary_operator(kind),
                    rhs,
                )
            } else {
                break;
            };
        }

        Ok(lhs)
    }

    /// Section 13.14 Conditional Expression
    /// `ConditionalExpression`[In, Yield, Await] :
    ///     `ShortCircuitExpression`[?In, ?Yield, ?Await]
    ///     `ShortCircuitExpression`[?In, ?Yield, ?Await] ? `AssignmentExpression`[+In, ?Yield, ?Await] : `AssignmentExpression`[?In, ?Yield, ?Await]
    fn parse_conditional_expression(&mut self) -> Result<Expression<'a>> {
        let span = self.start_span();
        let lhs = self.parse_binary_or_logical_expression_base(BindingPower::lowest())?;
        if !self.eat(Kind::Question) {
            return Ok(lhs);
        }

        let has_in = self.ctx.has_in();
        self.ctx = self.ctx.and_in(true);
        let consequent = self.parse_assignment_expression_base()?;
        self.ctx = self.ctx.and_in(has_in);

        self.expect(Kind::Colon)?;
        let alternate = self.parse_assignment_expression_base()?;
        Ok(self.ast.conditional_expression(self.end_span(span), lhs, consequent, alternate))
    }

    pub fn parse_assignment_expression_base(&mut self) -> Result<Expression<'a>> {
        match self.is_parenthesized_arrow_function() {
            IsParenthesizedArrowFunction::True => {
                return self.parse_parenthesized_arrow_function();
            }
            IsParenthesizedArrowFunction::Maybe => {
                let pos = self.cur_token().start;
                if !self.state.not_parenthesized_arrow.contains(&pos) {
                    if let Ok((type_parameters, params, return_type, r#async, span)) =
                        self.try_parse(Parser::parse_parenthesized_arrow_function_head)
                    {
                        return self.parse_arrow_function_body(
                            span,
                            type_parameters,
                            params,
                            return_type,
                            r#async,
                        );
                    }
                    self.state.not_parenthesized_arrow.insert(pos);
                }
            }
            IsParenthesizedArrowFunction::False => {}
        }

        let span = self.start_span();
        if self.cur_kind().is_binding_identifier()
            && self.peek_at(Kind::Arrow)
            && !self.peek_token().is_on_new_line
        {
            self.parse_single_param_function_expression(span, false, false)
        } else if self.at_async_no_new_line()
            && self.peek_kind().is_binding_identifier()
            && !self.peek_token().is_on_new_line
            && self.nth_at(2, Kind::Arrow)
            && !self.nth(2).is_on_new_line
        {
            self.bump_any(); // bump async
            self.parse_single_param_function_expression(span, true, false)
        } else {
            self.parse_assignment_expression()
        }
    }

    /// `AssignmentExpression`[In, Yield, Await] :
    pub fn parse_assignment_expression(&mut self) -> Result<Expression<'a>> {
        // [+Yield] YieldExpression
        if self.is_yield_expression() {
            return self.parse_yield_expression();
        }

        let span = self.start_span();

        let lhs = self.parse_conditional_expression()?;
        self.parse_assignment_expression_recursive(span, lhs)
    }

    fn parse_assignment_expression_recursive(
        &mut self,
        span: Span,
        lhs: Expression<'a>,
    ) -> Result<Expression<'a>> {
        if !self.cur_kind().is_assignment_operator() {
            return Ok(lhs);
        }

        let operator = map_assignment_operator(self.cur_kind());

        // 13.15.5 Destructuring Assignment
        // LeftHandSideExpression = AssignmentExpression
        // is converted to
        // AssignmentPattern[Yield, Await] :
        //    ObjectAssignmentPattern
        //    ArrayAssignmentPattern
        let left = AssignmentTarget::cover(lhs, self)?;

        self.bump_any();

        let right = self.parse_assignment_expression_base()?;
        Ok(self.ast.assignment_expression(self.end_span(span), operator, left, right))
    }

    /// Section 13.16 Sequence Expression
    fn parse_sequence_expression(
        &mut self,
        span: Span,
        first_expression: Expression<'a>,
    ) -> Result<Expression<'a>> {
        let mut expressions = self.ast.new_vec_single(first_expression);
        while self.eat(Kind::Comma) {
            let expression = self.parse_assignment_expression_base()?;
            expressions.push(expression);
        }
        Ok(self.ast.sequence_expression(self.end_span(span), expressions))
    }

    /// `AwaitExpression`[Yield] :
    ///     await `UnaryExpression`[?Yield, +Await]
    fn parse_await_expression(&mut self, lhs_span: Span) -> Result<Expression<'a>> {
        let span = self.start_span();
        self.bump_any();
        let has_await = self.ctx.has_await();
        self.ctx = self.ctx.and_await(true);
        let argument = self.parse_unary_expression_base(lhs_span)?;
        self.ctx = self.ctx.and_await(has_await);
        Ok(self.ast.await_expression(self.end_span(span), argument))
    }

    fn is_await_expression(&mut self) -> bool {
        if self.at(Kind::Await) {
            if self.ctx.has_await() {
                return true;
            }

            let peek_token = self.peek_token();
            // The following expressions are ambiguous
            // await + 0, await - 0, await ( 0 ), await [ 0 ], await / 0 /u, await ``, await of []
            if matches!(
                peek_token.kind,
                Kind::Of | Kind::LParen | Kind::LBrack | Kind::Slash | Kind::RegExp
            ) {
                return false;
            }

            return peek_token.kind.is_after_await_or_yield() && !peek_token.is_on_new_line;
        }
        false
    }

    fn is_yield_expression(&mut self) -> bool {
        if self.at(Kind::Yield) {
            if self.ctx.has_yield() {
                return true;
            }
            let peek_token = self.peek_token();
            return peek_token.kind.is_after_await_or_yield() && !peek_token.is_on_new_line;
        }
        false
    }
}
