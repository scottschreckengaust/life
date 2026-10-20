//! `select!` and `join!` without a proc-macro or a dependency.
//!
//! The trick: each recursive expansion writes the identifiers `__slot` and `__fut`,
//! and macro hygiene makes every one of them a different variable. The accumulator
//! carries those identifiers to the final rule, which can then name all N futures.

/// Wait on several futures; run the arm of the first one to finish. Losers are
/// dropped *before* the arm runs (so a `Drop` guard inside a loser sees the right time).
///
/// Branches are polled top to bottom, so on a tie the first branch wins. This
/// runtime is deterministic, so every `select!` is biased whether you say so or not.
/// Writing `biased;` is a statement of values.
#[macro_export]
macro_rules! select {
    (biased; $($rest:tt)+) => { $crate::select!(@munch [] $($rest)+) };
    (@munch [$($acc:tt)*] $p:pat = $f:expr => $b:expr $(, $($rest:tt)*)?) => {
        $crate::select!(@munch [$($acc)* (__slot __fut ($p) ($f) ($b))] $($($rest)*)?)
    };
    (@munch [$(($s:ident $fu:ident ($p:pat) ($f:expr) ($b:expr)))+]) => {{
        $( let mut $s = ::core::option::Option::None; )+
        {
            $( let mut $fu = ::core::pin::pin!($f); )+
            ::core::future::poll_fn(|cx| {
                $(
                    if let ::core::task::Poll::Ready(v) = ::core::future::Future::poll($fu.as_mut(), cx) {
                        $s = ::core::option::Option::Some(v);
                        return ::core::task::Poll::Ready(());
                    }
                )+
                ::core::task::Poll::Pending
            })
            .await;
        }
        $( if let ::core::option::Option::Some($p) = $s { $b } else )+ { ::core::unreachable!() }
    }};
    ($($rest:tt)+) => { $crate::select!(@munch [] $($rest)+) };
}

/// Run several futures concurrently and wait for all of them. Returns a tuple.
#[macro_export]
macro_rules! join {
    (@munch [$($acc:tt)*] $f:expr $(, $($rest:tt)*)?) => {
        $crate::join!(@munch [$($acc)* (__slot __fut ($f))] $($($rest)*)?)
    };
    (@munch [$(($s:ident $fu:ident ($f:expr)))+]) => {{
        $( let mut $s = ::core::option::Option::None; )+
        {
            $( let mut $fu = ::core::pin::pin!($f); )+
            ::core::future::poll_fn(|cx| {
                let mut done = true;
                $(
                    if $s.is_none() {
                        match ::core::future::Future::poll($fu.as_mut(), cx) {
                            ::core::task::Poll::Ready(v) => $s = ::core::option::Option::Some(v),
                            ::core::task::Poll::Pending => done = false,
                        }
                    }
                )+
                if done { ::core::task::Poll::Ready(()) } else { ::core::task::Poll::Pending }
            })
            .await;
        }
        ($( $s.unwrap(), )+)
    }};
    ($($f:tt)+) => { $crate::join!(@munch [] $($f)+) };
}
