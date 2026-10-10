//! 핸들링 설정. 브라우저 localStorage에 저장한다.

use tetrust_engine::{Config, Millis};

const STORAGE_KEY: &str = "tetrust.handling";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handling {
    pub das: Millis,
    pub arr: Millis,
    /// 0이면 바닥까지 즉시.
    pub soft_drop: Millis,
}

impl Default for Handling {
    fn default() -> Self {
        let c = Config::default();
        Self {
            das: c.das,
            arr: c.arr,
            soft_drop: c.soft_drop,
        }
    }
}

/// 누구에게나 같은 범위. 끝값(ARR 0 = 즉시)도 모두에게 열려 있다.
pub const DAS_RANGE: (Millis, Millis) = (0, 500);
pub const ARR_RANGE: (Millis, Millis) = (0, 200);
pub const SOFT_DROP_RANGE: (Millis, Millis) = (0, 200);

fn clamp(v: Millis, (lo, hi): (Millis, Millis)) -> Millis {
    v.clamp(lo, hi)
}

impl Handling {
    pub fn clamped(self) -> Self {
        Self {
            das: clamp(self.das, DAS_RANGE),
            arr: clamp(self.arr, ARR_RANGE),
            soft_drop: clamp(self.soft_drop, SOFT_DROP_RANGE),
        }
    }

    pub fn config(self) -> Config {
        Config {
            das: self.das,
            arr: self.arr,
            soft_drop: self.soft_drop,
            ..Config::default()
        }
    }

    fn encode(self) -> String {
        format!("{},{},{}", self.das, self.arr, self.soft_drop)
    }

    fn decode(s: &str) -> Option<Self> {
        let mut it = s.split(',').map(|p| p.trim().parse::<Millis>());
        let h = Self {
            das: it.next()?.ok()?,
            arr: it.next()?.ok()?,
            soft_drop: it.next()?.ok()?,
        };
        Some(h.clamped())
    }

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    pub fn load() -> Self {
        Self::storage()
            .and_then(|s| s.get_item(STORAGE_KEY).ok()?)
            .and_then(|v| Self::decode(&v))
            .unwrap_or_default()
    }

    pub fn save(self) {
        if let Some(s) = Self::storage() {
            let _ = s.set_item(STORAGE_KEY, &self.encode());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_clamps() {
        let h = Handling {
            das: 100,
            arr: 0,
            soft_drop: 20,
        };
        assert_eq!(Handling::decode(&h.encode()), Some(h));
        assert_eq!(
            Handling::decode("9999,7,5"),
            Some(Handling {
                das: 500,
                arr: 7,
                soft_drop: 5
            })
        );
        assert_eq!(Handling::decode("garbage"), None);
        assert_eq!(Handling::decode("1,2"), None);
    }
}
