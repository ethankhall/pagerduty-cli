use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::RwLock;

#[derive(Debug)]
struct Line {
    id: usize,
    content: String,
}

impl Line {
    fn print(&self, prefix: &str, is_last: bool, graph: &Graph) -> String {
        let mut output_buffer = String::default();

        if is_last {
            output_buffer += &format!("{} └─ {}\n", prefix, self.content);
        } else {
            output_buffer += &format!("{} ├─ {}\n", prefix, self.content);
        }

        let child_prefix = if is_last {
            format!("{}   ", prefix)
        } else {
            format!("{} │ ", prefix)
        };

        let edges = graph.edges.read().unwrap();
        let edges = edges.get(&self.id).unwrap();
        let size = edges.len();
        for (idx, node) in edges.iter().enumerate() {
            let line = graph.get_line(*node);
            output_buffer += &line.print(&child_prefix, idx == size - 1, &graph);
        }

        output_buffer
    }
}

#[derive(Debug)]
struct Graph {
    nodes: RwLock<BTreeMap<usize, Rc<Line>>>,
    edges: RwLock<BTreeMap<usize, Vec<usize>>>,
    index_counter: RwLock<usize>,
}

impl Graph {
    fn add_line(&self, message: String) -> usize {
        {
            *self.index_counter.write().unwrap() += 1;
        }
        let id = *self.index_counter.read().unwrap();
        let line = Line {
            id,
            content: message,
        };
        self.nodes.write().unwrap().insert(id, Rc::new(line));
        self.edges.write().unwrap().insert(id, Vec::new());
        id
    }

    fn connect_edges(&self, parent: usize, child: usize) {
        self.edges
            .write()
            .unwrap()
            .entry(parent)
            .or_default()
            .push(child);
    }

    fn get_line(&self, id: usize) -> Rc<Line> {
        self.nodes.read().unwrap().get(&id).unwrap().clone()
    }
}

impl std::default::Default for Graph {
    fn default() -> Self {
        Graph {
            nodes: Default::default(),
            edges: Default::default(),
            index_counter: Default::default(),
        }
    }
}

pub struct OutputLine {
    graph: Rc<Graph>,
    id: usize,
}

impl OutputLine {
    pub fn add_line(&self, message: String) -> OutputLine {
        let child_id = self.graph.add_line(message);
        self.graph.connect_edges(self.id, child_id);

        OutputLine {
            graph: self.graph.clone(),
            id: child_id,
        }
    }
}

pub struct TreePrinter {
    graph: Rc<Graph>,
    roots: RwLock<Vec<usize>>,
}

impl std::default::Default for TreePrinter {
    fn default() -> Self {
        TreePrinter {
            graph: Default::default(),
            roots: Default::default(),
        }
    }
}

impl TreePrinter {
    pub fn add_line(&self, line: String) -> OutputLine {
        let id = self.graph.add_line(line);
        {
            self.roots.write().unwrap().push(id);
        }

        OutputLine {
            graph: self.graph.clone(),
            id,
        }
    }

    pub fn render(&self) -> String {
        let size = self.roots.read().unwrap().len();
        let mut buffer = String::default();

        for (idx, root) in self.roots.read().unwrap().iter().enumerate() {
            let line = self.graph.get_line(*root);
            buffer += &line.print("", idx == size - 1, &self.graph);
        }

        buffer
    }
}
