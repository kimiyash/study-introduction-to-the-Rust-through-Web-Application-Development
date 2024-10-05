macro_rules! sum {
    ( $( $x:expr ),* ) => {
        {
            let mut result = 0;
            $(
                result += $x;
            )*
            result
        }
    }
}

fn main() {
    let sum = sum![1, 2, 3, 4];
    println!("{}", sum);
}