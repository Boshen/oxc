use oxc_ast::{AstKind, Span};
use oxc_diagnostics::{
    miette::{self, Diagnostic},
    thiserror::Error,
};
use oxc_macros::declare_oxc_lint;

use crate::{context::LintContext, rule::Rule, AstNode};

#[derive(Debug, Error, Diagnostic)]
#[error("eslint(no-debugger): `debugger` statement is not allowed")]
#[diagnostic()]
struct NoDebuggerDiagnostic(#[label] pub Span);

#[derive(Debug, Default, Clone)]
pub struct NoDebugger;

declare_oxc_lint!(
    /// ### What it does
    /// Checks for usage of the `debugger` statement
    ///
    /// ### Why is this bad?
    /// `debugger` statements do not affect functionality when a debugger isn't attached.
    /// They're most commonly an accidental debugging leftover.
    ///
    ///
    /// ### Example
    /// ```javascript
    /// const data = await getData();
    /// const result = complexCalculation(data);
    /// debugger;
    /// ```
    NoDebugger
);

const RULE_NAME: &str = "no-debugger";

impl Rule for NoDebugger {
    const NAME: &'static str = RULE_NAME;

    fn run<'a>(&self, node: &AstNode<'a>, ctx: &LintContext<'a>) {
        if let AstKind::DebuggerStatement(stmt) = node.get().kind() {
            ctx.diagnostic(NoDebuggerDiagnostic(stmt.span));
        }
    }
}

#[test]
fn test() {
    use crate::tester::Tester;

    let pass = vec![("var test = { debugger: 1 }; test.debugger;", None)];

    let fail = vec![("if (foo) debugger", None)];

    Tester::new(RULE_NAME, pass, fail).test_and_snapshot();
}