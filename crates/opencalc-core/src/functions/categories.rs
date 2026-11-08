//! Which group of the Functions window each built-in function is listed under.

/// The group a function is listed under (the Functions window and its search).
pub fn category(name: &str) -> &'static str {
    match name {
        "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "atan2" | "sinh" | "cosh" | "tanh"
        | "asinh" | "acosh" | "atanh" => "Trigonometry",
        "ln" | "exp" | "sqrt" | "log" => "Powers and logarithms",
        "floor" | "ceil" | "trunc" | "round" | "sign" | "mod" | "gcd" | "lcm" => {
            "Rounding and integers"
        }
        "abs" | "re" | "im" | "conj" | "arg" => "Complex numbers",
        "vec" | "mat" | "transpose" | "det" | "norm" | "dot" | "rows" | "cols" | "identity"
        | "tr" | "rank" | "cross" | "augment" | "stack" => "Vectors and matrices",
        "sum" | "mean" | "stdev" | "stdev_sample" | "min" | "max" | "length" | "median"
        | "mode" | "var" | "var_sample" | "sort" | "reverse" | "cumsum" | "cvar" | "corr" => {
            "Statistics"
        }
        "factorial" | "combin" | "permut" | "isprime" => "Number theory",
        "sum_over" | "product_over" | "integrate" | "derivative" | "limit" => "Calculus",
        "root" | "polyroots" | "find" | "minerr" | "minimize" | "maximize" | "odesolve" => {
            "Solving"
        }
        "col" | "row" => "Vectors and matrices",
        _ => "Other",
    }
}
