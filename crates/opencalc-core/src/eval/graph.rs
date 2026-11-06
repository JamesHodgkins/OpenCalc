//! Strongly connected components of the dependency graph.

/// Tarjan's SCC over `edges[i]` = nodes that `i` points to. Components are
/// returned in reverse topological order (targets before sources).
pub(super) fn tarjan(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    struct State<'a> {
        edges: &'a [Vec<usize>],
        index: Vec<Option<usize>>,
        low: Vec<usize>,
        on_stack: Vec<bool>,
        stack: Vec<usize>,
        next: usize,
        out: Vec<Vec<usize>>,
    }

    fn visit(s: &mut State, v: usize) {
        s.index[v] = Some(s.next);
        s.low[v] = s.next;
        s.next += 1;
        s.stack.push(v);
        s.on_stack[v] = true;
        for k in 0..s.edges[v].len() {
            let w = s.edges[v][k];
            match s.index[w] {
                None => {
                    visit(s, w);
                    s.low[v] = s.low[v].min(s.low[w]);
                }
                Some(wi) if s.on_stack[w] => s.low[v] = s.low[v].min(wi),
                Some(_) => {}
            }
        }
        if Some(s.low[v]) == s.index[v] {
            let mut comp = Vec::new();
            while let Some(w) = s.stack.pop() {
                s.on_stack[w] = false;
                comp.push(w);
                if w == v {
                    break;
                }
            }
            comp.reverse();
            s.out.push(comp);
        }
    }

    let n = edges.len();
    let mut s = State {
        edges,
        index: vec![None; n],
        low: vec![0; n],
        on_stack: vec![false; n],
        stack: Vec::new(),
        next: 0,
        out: Vec::new(),
    };
    for v in 0..n {
        if s.index[v].is_none() {
            visit(&mut s, v);
        }
    }
    s.out
}
