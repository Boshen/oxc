pub mod early_error {
    pub mod javascript;
}

oxc_macros::declare_all_lint_rules! {
    constructor_super,
    eq_eq_eq,
    for_direction,
    no_debugger,
    no_duplicate_case,
    no_array_constructor,
    no_async_promise_executor,
    no_caller,
    no_empty,
    no_empty_pattern,
    no_self_compare,
    no_mixed_operators,
    no_constant_binary_expression,
    deepscan::uninvoked_array_callback,
    use_isnan,
}
