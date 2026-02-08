#[macro_export]
#[doc(hidden)]
macro_rules! pzip_part {
    (numeric $path:literal $params:ident) => {{
        use $crate::parameters::BufferStates;
        $crate::parameters::decompose_numeric(
            $params
                .numeric_by_hash(const { $crate::parameters::hash_id($path) })
                .unwrap(),
        )
    }};
    (enum $path:literal $params:ident) => {{
        use $crate::parameters::BufferStates;
        $crate::parameters::decompose_enum(
            $params
                .enum_by_hash(const { $crate::parameters::hash_id($path) })
                .unwrap(),
        )
    }};
    (switch $path:literal $params:ident) => {{
        use $crate::parameters::BufferStates;
        $crate::parameters::decompose_switch(
            $params
                .switch_by_hash(const { $crate::parameters::hash_id($path) })
                .unwrap(),
        )
    }};
    (global_expression_numeric $variant:ident $params:ident) => {{
        use $crate::synth::{NumericGlobalExpression, SynthParamBufferStates};
        $crate::parameters::decompose_numeric(
            $params.get_numeric_global_expression(NumericGlobalExpression::$variant),
        )
    }};
    (global_expression_switch $variant:ident $params:ident) => {{
        use $crate::synth::{SwitchGlobalExpression, SynthParamBufferStates};
        $crate::parameters::decompose_switch(
            $params.get_switch_global_expression(SwitchGlobalExpression::$variant),
        )
    }};
    (external_numeric $expr:tt $params:ident) => {{ $crate::parameters::decompose_numeric($expr) }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! pzip_value_type {
    (numeric) => {
        f32
    };
    (enum) => {
        u32
    };
    (switch) => {
        bool
    };
    (global_expression_numeric) => {
        f32
    };
    (global_expression_switch) => {
        bool
    };
    (external_numeric) => {
        f32
    };
}

#[macro_export]
#[doc(hidden)]
macro_rules! pzip_collect {
    // Base case: Generate the struct and function
    (
        $params:ident,
        [], // No more inputs
        [ $($names:ident,)* ], // Remaining names
        [ $($acc_name:ident $acc_kind:ident $acc_path:tt)* ] // Accumulated
    ) => {
        {
            #[allow(unused_parens, non_snake_case, clippy::too_many_arguments)]
            fn pzip_impl<
                $($acc_name: Iterator<Item = $crate::pzip_value_type!($acc_kind)> + Clone),*
            >(
                $($acc_name: ($crate::pzip_value_type!($acc_kind), Option<$acc_name>)),*
            ) -> impl Iterator<Item = ($($crate::pzip_value_type!($acc_kind)),*)> + Clone {
                #[derive(Clone, Copy)]
                #[allow(non_snake_case)]
                struct Values<$($acc_name: Copy),*> {
                    $($acc_name: $acc_name),*
                }

                #[derive(Clone)]
                #[allow(non_snake_case)]
                struct Iters<$($acc_name),*> {
                    $($acc_name: Option<$acc_name>),*
                }

                struct PZipIter<$($acc_name),*> {
                    values: Values<$($crate::pzip_value_type!($acc_kind)),*>,
                    iters: Iters<$($acc_name),*>,
                    mask: u64,
                }

                impl<$($acc_name: Clone),*> Clone for PZipIter<$($acc_name),*> {
                    fn clone(&self) -> Self {
                        PZipIter {
                            values: self.values,
                            iters: Iters { $($acc_name: self.iters.$acc_name.clone()),* },
                            mask: self.mask,
                        }
                    }
                }

                impl<$($acc_name: Iterator<Item = $crate::pzip_value_type!($acc_kind)> + Clone),*> Iterator for PZipIter<$($acc_name),*> {
                    #[allow(unused_parens)]
                    type Item = ($($crate::pzip_value_type!($acc_kind)),*);

                    #[inline(always)]
                    fn next(&mut self) -> Option<Self::Item> {
                        {
                            let mut _bit = 1u64;
                            $(
                                if self.mask & _bit != 0 {
                                    self.values.$acc_name = self.iters.$acc_name.as_mut().unwrap().next()?;
                                }
                                _bit <<= 1;
                            )*
                        }
                        Some(($(self.values.$acc_name),*))
                    }
                }

                let mut mask = 0u64;
                {
                    let mut _bit = 1u64;
                    $(
                        if $acc_name.1.is_some() {
                            mask |= _bit;
                        }
                        _bit <<= 1;
                    )*
                }

                PZipIter {
                    values: Values { $($acc_name: $acc_name.0),* },
                    iters: Iters { $($acc_name: $acc_name.1),* },
                    mask,
                }
            }

            pzip_impl(
                $( $crate::pzip_part!($acc_kind $acc_path $params) ),*
            )
        }
    };

    // Recursive step
    (
        $params:ident,
        [ $k:ident $p:tt $(, $rest_k:ident $rest_p:tt)* ],
        [ $next_name:ident, $($rest_names:ident,)* ],
        [ $($acc:tt)* ]
    ) => {
        $crate::pzip_collect!(
            $params,
            [ $($rest_k $rest_p),* ],
            [ $($rest_names,)* ],
            [ $($acc)* $next_name $k $p ]
        )
    };
}

/// Utility to get a per-sample iterator including the state of multiple parameters.
///
/// This is a convenient way to consume a [`BufferStates`] object if you intend
/// to track the per-sample state of multiple parameters.
///
/// This macro indexes into a [`BufferStates`] object with a list of parameter
/// ids and their types. See the examples below for usage.
///
/// # Examples
///
/// ```
/// # use conformal_component::pzip;
/// # use conformal_component::parameters::{ConstantBufferStates, StaticInfoRef, TypeSpecificInfoRef, InternalValue};
/// let params = ConstantBufferStates::new_defaults(
///   vec![
///     StaticInfoRef {
///       title: "Numeric",
///       short_title: "Numeric",
///       unique_id: "gain",
///       flags: Default::default(),
///       type_specific: TypeSpecificInfoRef::Numeric {
///         default: 0.0,
///         valid_range: 0.0..=1.0,
///         units: None,
///       },
///     },
///     StaticInfoRef {
///       title: "Enum",
///       short_title: "Enum",
///       unique_id: "letter",
///       flags: Default::default(),
///       type_specific: TypeSpecificInfoRef::Enum {
///         default: 1,
///         values: &["A", "B", "C"],
///       },
///     },
///     StaticInfoRef {
///       title: "Switch",
///       short_title: "Switch",
///       unique_id: "my special switch",
///       flags: Default::default(),
///       type_specific: TypeSpecificInfoRef::Switch {
///         default: false,
///       },
///     },
///   ],
/// );
///
/// let samples: Vec<_> = pzip!(params[
///   numeric "gain",
///   enum "letter",
///   switch "my special switch"
/// ]).take(2).collect();
///
/// assert_eq!(samples, vec![(0.0, 1, false), (0.0, 1, false)]);
/// ```
///
/// # Global Expression Parameters
///
/// For synths, you can also access global expression controllers
/// using `global_expression_numeric` and `global_expression_switch`.
/// Note that this requires the parameter source to implement
/// [`SynthParamBufferStates`](crate::synth::SynthParamBufferStates).
///
/// ```
/// # use conformal_component::pzip;
/// # use conformal_component::parameters::{ConstantBufferStates, StaticInfoRef, TypeSpecificInfoRef, InternalValue};
/// # use conformal_component::synth::SynthParamBufferStates;
/// let params = ConstantBufferStates::new_synth_defaults(
///   vec![
///     StaticInfoRef {
///       title: "Gain",
///       short_title: "Gain",
///       unique_id: "gain",
///       flags: Default::default(),
///       type_specific: TypeSpecificInfoRef::Numeric {
///         default: 0.5,
///         valid_range: 0.0..=1.0,
///         units: None,
///       },
///     },
///   ],
/// );
///
/// let samples: Vec<_> = pzip!(params[
///   numeric "gain",
///   global_expression_numeric ModWheel,
///   global_expression_switch SustainPedal
/// ]).take(1).collect();
///
/// assert_eq!(samples, vec![(0.5, 0.0, false)]);
/// ```
///
/// # External Numeric Parameters
///
/// You can also inject a [`NumericBufferState`](crate::parameters::NumericBufferState)
/// from outside the params object using `external_numeric`. The expression must
/// be wrapped in parentheses.
///
/// ```
/// # use conformal_component::pzip;
/// # use conformal_component::parameters::{ConstantBufferStates, StaticInfoRef, TypeSpecificInfoRef, InternalValue, NumericBufferState};
/// let params = ConstantBufferStates::new_defaults(
///   vec![
///     StaticInfoRef {
///       title: "Gain",
///       short_title: "Gain",
///       unique_id: "gain",
///       flags: Default::default(),
///       type_specific: TypeSpecificInfoRef::Numeric {
///         default: 0.5,
///         valid_range: 0.0..=1.0,
///         units: None,
///       },
///     },
///   ],
/// );
///
/// let external: NumericBufferState<std::iter::Empty<_>> = NumericBufferState::Constant(0.75);
/// let samples: Vec<_> = pzip!(params[
///   numeric "gain",
///   external_numeric (external)
/// ]).take(2).collect();
///
/// assert_eq!(samples, vec![(0.5, 0.75), (0.5, 0.75)]);
/// ```
#[macro_export]
macro_rules! pzip {
    ($params:ident[$($kind:ident $path:tt),+]) => {
        $crate::pzip_collect!(
            $params,
            [ $($kind $path),+ ],
            [ P0, P1, P2, P3, P4, P5, P6, P7, P8, P9, P10, P11, P12, P13, P14, P15, P16, P17, P18, P19, P20, P21, P22, P23, P24, P25, P26, P27, P28, P29, P30, P31, P32, P33, P34, P35, P36, P37, P38, P39, P40, P41, P42, P43, P44, P45, P46, P47, P48, P49, P50, P51, P52, P53, P54, P55, P56, P57, P58, P59, P60, P61, P62, P63, P64, P65, P66, P67, P68, P69, P70, P71, P72, P73, P74, P75, P76, P77, P78, P79, P80, P81, P82, P83, P84, P85, P86, P87, P88, P89, P90, P91, P92, P93, P94, P95, P96, P97, P98, P99, P100, P101, P102, P103, P104, P105, P106, P107, P108, P109, P110, P111, P112, P113, P114, P115, P116, P117, P118, P119, P120, P121, P122, P123, P124, P125, P126, P127, P128, P129, P130, P131, P132, P133, P134, P135, P136, P137, P138, P139, P140, P141, P142, P143, P144, P145, P146, P147, P148, P149, P150, P151, P152, P153, P154, P155, P156, P157, P158, P159, P160, P161, P162, P163, P164, P165, P166, P167, P168, P169, P170, P171, P172, P173, P174, P175, P176, P177, P178, P179, P180, P181, P182, P183, P184, P185, P186, P187, P188, P189, P190, P191, P192, P193, P194, P195, P196, P197, P198, P199, P200, P201, P202, P203, P204, P205, P206, P207, P208, P209, P210, P211, P212, P213, P214, P215, P216, P217, P218, P219, P220, P221, P222, P223, P224, P225, P226, P227, P228, P229, P230, P231, P232, P233, P234, P235, P236, P237, P238, P239, P240, P241, P242, P243, P244, P245, P246, P247, P248, P249, P250, P251, P252, P253, P254, P255, ],
            []
        )
    };
    ($expr_head:ident $(. $expr_part:ident $( ( $($args:tt)* ) )? )+ [$($kind:ident $path:tt),+]) => {
        {
            let __pzip_params = $expr_head $(. $expr_part $( ( $($args)* ) )? )+;
            $crate::pzip!(__pzip_params[$($kind $path),+])
        }
    };
}

/// Grab an instantaneous snapshot of parameter values at the start of the buffer.
///
/// This has the same syntax as [`pzip!`] but instead of returning a per-sample
/// iterator, it returns the values from the first sample as a tuple.
///
/// This is useful for parameters that don't need to be modulated every
/// sample.
///
/// # Examples
///
/// ```
/// # use conformal_component::pgrab;
/// # use conformal_component::parameters::{ConstantBufferStates, StaticInfoRef, TypeSpecificInfoRef, InternalValue};
/// let params = ConstantBufferStates::new_defaults(
///   vec![
///     StaticInfoRef {
///       title: "Gain",
///       short_title: "Gain",
///       unique_id: "gain",
///       flags: Default::default(),
///       type_specific: TypeSpecificInfoRef::Numeric {
///         default: 0.5,
///         valid_range: 0.0..=1.0,
///         units: None,
///       },
///     },
///     StaticInfoRef {
///       title: "Switch",
///       short_title: "Switch",
///       unique_id: "enabled",
///       flags: Default::default(),
///       type_specific: TypeSpecificInfoRef::Switch {
///         default: true,
///       },
///     },
///   ],
/// );
///
/// let (gain, enabled) = pgrab!(params[numeric "gain", switch "enabled"]);
/// assert_eq!(gain, 0.5);
/// assert_eq!(enabled, true);
/// ```
///
/// It also works with a single parameter:
///
/// ```
/// # use conformal_component::pgrab;
/// # use conformal_component::parameters::{ConstantBufferStates, StaticInfoRef, TypeSpecificInfoRef, InternalValue};
/// let params = ConstantBufferStates::new_defaults(
///   vec![
///     StaticInfoRef {
///       title: "Gain",
///       short_title: "Gain",
///       unique_id: "gain",
///       flags: Default::default(),
///       type_specific: TypeSpecificInfoRef::Numeric {
///         default: 0.75,
///         valid_range: 0.0..=1.0,
///         units: None,
///       },
///     },
///   ],
/// );
///
/// let gain = pgrab!(params[numeric "gain"]);
/// assert_eq!(gain, 0.75);
/// ```
#[macro_export]
macro_rules! pgrab {
    ($params:ident[$($kind:ident $path:tt),+]) => {
        $crate::pzip!($params[$($kind $path),+]).next().unwrap()
    };
    ($expr_head:ident $(. $expr_part:ident $( ( $($args:tt)* ) )? )+ [$($kind:ident $path:tt),+]) => {
        {
            let __pgrab_params = $expr_head $(. $expr_part $( ( $($args)* ) )? )+;
            $crate::pgrab!(__pgrab_params[$($kind $path),+])
        }
    };
}
