use cow_utils::CowUtils;
use rustc_hash::{FxHashMap, FxHashSet};

use oxc_diagnostics::OxcDiagnostic;
use oxc_str::Ident;

use crate::diagnostics;
use crate::react_compiler_hir::environment::Environment;
use crate::react_compiler_hir::{
    HirFunction, IdentifierId, InstructionValue, NonLocalBinding, PropertyLiteral,
};
use oxc_span::Span;

/// Standard JavaScript built-in objects. Their capitalized members (`Date.UTC`,
/// `Intl.DateTimeFormat`, `Intl.NumberFormat`) are part of the language, never
/// components, so calling them is not a capitalized call.
///
/// Divergence from upstream ValidateNoCapitalizedCalls.ts, which flags every
/// capitalized method call whatever its receiver.
const BUILT_IN_OBJECTS: &[&str] = &[
    "Array",
    "ArrayBuffer",
    "Atomics",
    "BigInt",
    "Date",
    "Intl",
    "JSON",
    "Math",
    "Number",
    "Object",
    "Promise",
    "Proxy",
    "Reflect",
    "String",
    "Symbol",
    "Temporal",
    "WebAssembly",
];

/// Validates that capitalized functions are not called directly (they should be rendered as JSX).
///
/// Port of ValidateNoCapitalizedCalls.ts.
pub fn validate_no_capitalized_calls(
    func: &HirFunction,
    env: &mut Environment,
) -> Result<(), OxcDiagnostic> {
    // Build the allow list from global registry keys + config entries
    let mut allow_list: FxHashSet<String> = env.globals().keys().map(str::to_owned).collect();
    if let Some(config_entries) = &env.config.validate_no_capitalized_calls {
        for entry in config_entries {
            allow_list.insert(entry.clone());
        }
    }

    let mut capital_load_globals: FxHashMap<IdentifierId, Ident> = FxHashMap::default();
    let mut built_in_objects: FxHashSet<IdentifierId> = FxHashSet::default();
    let mut capitalized_properties: FxHashMap<IdentifierId, (Ident, Option<Span>)> =
        FxHashMap::default();

    for (_block_id, block) in &func.body.blocks {
        for &instr_id in &block.instructions {
            let instr = &func.instructions[instr_id.index()];
            let lvalue_id = instr.lvalue.identifier;
            let value = &instr.value;

            match value {
                InstructionValue::LoadGlobal { binding, .. } => {
                    let name = binding.name();
                    // Only a free global is the built-in: an import or a module-level
                    // binding named `Date` is the module's own value.
                    if matches!(binding, NonLocalBinding::Global { .. })
                        && BUILT_IN_OBJECTS.contains(&name.as_str())
                    {
                        built_in_objects.insert(lvalue_id);
                    }
                    if !name.is_empty()
                        && name.starts_with(|c: char| c.is_ascii_uppercase())
                        // We don't want to flag CONSTANTS()
                        && name.as_str() != name.cow_to_uppercase()
                        && !allow_list.contains(name.as_str())
                    {
                        capital_load_globals.insert(lvalue_id, name);
                    }
                }
                InstructionValue::CallExpression { callee, .. } => {
                    let callee_id = callee.identifier;
                    if let Some(callee_name) = capital_load_globals.get(&callee_id) {
                        env.record_error(diagnostics::capitalized_call(callee_name, callee.span))?;
                        continue;
                    }
                }
                InstructionValue::PropertyLoad {
                    object,
                    property: PropertyLiteral::String(prop_name),
                    property_span,
                    ..
                } => {
                    if prop_name.starts_with(|c: char| c.is_ascii_uppercase())
                        && !built_in_objects.contains(&object.identifier)
                    {
                        capitalized_properties.insert(lvalue_id, (*prop_name, *property_span));
                    }
                }
                InstructionValue::MethodCall { property, .. } => {
                    let property_id = property.identifier;
                    if let Some((prop_name, property_span)) =
                        capitalized_properties.get(&property_id)
                    {
                        env.record_error(diagnostics::capitalized_call(
                            prop_name,
                            property_span.or(property.span),
                        ))?;
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}
