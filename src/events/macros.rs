// Define a payload-less enum with baked-in metadata.
//
// Usage: see `defs.rs`
// define_operator_events! {
//     pub enum OperatorEvent {
//         EboCreated => { message: "New EboServer", level: Info, eventhub: true, k8s: true },
//         // ...
//     }
// }

#[macro_export]
macro_rules! define_operator_events {
    (
        $(#[$meta:meta])*
        $vis:vis enum $EnumName:ident {
            $(
                $(#[$vmeta:meta])*
                $Variant:ident
                    => { message: $msg:expr, level: $lvl:ident, k8s: $k8s:expr }
            ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
        $vis enum $EnumName {
            $( $(#[$vmeta])* $Variant, )+
        }

        impl $EnumName {
            /// Human-readable message text for this event kind.
            #[inline]
            pub fn message(self) -> &'static str {
                match self { $( Self::$Variant => $msg, )+ }
            }

            /// Stable enum/variant name like "EboCreated".
            #[inline]
            pub fn kind(self) -> &'static str {
                match self { $( Self::$Variant => stringify!($Variant), )+ }
            }

            /// Level attached to this event kind.
            #[inline]
            pub fn level(self) -> $crate::events::Level {
                match self { $( Self::$Variant => $crate::events::Level::$lvl, )+ }
            }

            /// Whether we emit a Kubernetes Event for this kind.
            #[inline]
            pub fn send_to_k8s(self) -> bool {
                match self { $( Self::$Variant => $k8s, )+ }
            }
        }

        impl ::core::fmt::Display for $EnumName {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                write!(f, "{}", self.kind())
            }
        }
    };
}
