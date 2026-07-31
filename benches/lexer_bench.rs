use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use lyzard::lexer::Lexer;

// A realistic LYZARD program for benchmarking
const BENCH_SRC: &str = r#"
fn fibonacci(n: int) -> int {
    if n <= 1 {
        return n
    }
    return fibonacci(n - 1) + fibonacci(n - 2)
}

struct Matrix {
    rows: int,
    cols: int,
    data: [float],
}

impl Matrix {
    fn new(rows: int, cols: int) -> Matrix {
        return Matrix { rows: rows, cols: cols, data: [] }
    }

    fn get(self, row: int, col: int) -> float {
        return self.data[row * self.cols + col]
    }

    fn set(self, row: int, col: int, val: float) {
        self.data[row * self.cols + col] = val
    }
}

fn main() {
    let n = 30
    let result = fibonacci(n)
    print("fib(30) = ")
    print(result)

    let m = Matrix.new(100, 100)
    for i in 0..100 {
        for j in 0..100 {
            m.set(i, j, 1.0 / (i + j + 1) as float)
        }
    }
}
"#;

fn bench_lexer(c: &mut Criterion) {
    let src_bytes = BENCH_SRC.len() as u64;

    let mut group = c.benchmark_group("lexer");
    group.throughput(Throughput::Bytes(src_bytes));

    group.bench_function("tokenize", |b| {
        b.iter(|| {
            let tokens = Lexer::tokenize(black_box(BENCH_SRC), "bench.lyz").unwrap();
            black_box(tokens)
        })
    });

    group.finish();
}

fn bench_lexer_large(c: &mut Criterion) {
    // Generate a large source file: repeat BENCH_SRC 100 times
    let large_src = BENCH_SRC.repeat(100);
    let src_bytes = large_src.len() as u64;

    let mut group = c.benchmark_group("lexer_large");
    group.throughput(Throughput::Bytes(src_bytes));
    group.sample_size(20); // fewer samples for large input

    group.bench_function("tokenize_100x", |b| {
        b.iter(|| {
            let tokens = Lexer::tokenize(black_box(&large_src), "bench_large.lyz").unwrap();
            black_box(tokens)
        })
    });

    group.finish();
}

criterion_group!(benches, bench_lexer, bench_lexer_large);
criterion_main!(benches);
