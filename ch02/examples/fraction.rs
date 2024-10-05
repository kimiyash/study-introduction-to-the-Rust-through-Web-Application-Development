use std::fmt::Display;
use std::mem::swap;
use std::ops::Add;

#[derive(Debug, PartialEq, Eq)]
struct Fraction(u32, u32);

impl Fraction {
    fn new(numerator: u32, denominator: u32) -> Self {
        let gcd = Self::gcd(numerator, denominator);
        Self(numerator / gcd, denominator / gcd)
    }

    fn gcd(mut a: u32, mut b: u32) -> u32 {
        if a < b {
            swap(&mut a, &mut b)
        }
        if b == 0 {
            a
        } else {
            Self::gcd(b, a % b)
        }
    }
}

impl Add for Fraction {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        // // お互いの分母を書けて最大公約数で割ることで分母の最小公倍数を求める
        // let dlcm = self.1 * rhs.1 / Self::gcd(self.1, rhs.1);
        // // 最小公倍数を分母で割った数で分子に掛ける
        // let numerator = self.0 * (dlcm / self.1) + rhs.0 * (dlcm / rhs.1);
        // Fraction::new(numerator, dlcm)

        // どうせ約分するんだしこっちでよくない？
        let denominator = self.1 * rhs.1;
        let numerator = self.0 * rhs.1 + rhs.0 * self.1;
        Fraction::new(numerator, denominator)
    }
}

impl Display for Fraction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.0, self.1)
    }
}

fn main() {
    assert_eq!(Fraction::new(10, 30), Fraction::new(1, 3));
    assert_eq!(Fraction::new(30, 10), Fraction::new(3, 1));
    println!("{}", Fraction::new(56, 104));
    assert_eq!(Fraction::new(56, 104), Fraction::new(7, 13));
    println!("{}", Fraction::new(10, 30) + Fraction::new(30, 10));
    assert_eq!(
        Fraction::new(10, 30) + Fraction::new(30, 10),
        Fraction::new(100, 30)
    );
    println!("{}", Fraction::new(2, 3) + Fraction::new(4, 5));
    assert_eq!(
        Fraction::new(2, 3) + Fraction::new(4, 5),
        Fraction::new(22, 15)
    );
}
