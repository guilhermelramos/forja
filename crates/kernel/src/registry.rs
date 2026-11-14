//! Registrar and registry: the single place where modules declare everything.

use crate::args::{Args, CommandId};
use crate::commands::{Availability, CommandField, CommandSpec};
use crate::error::{KernelError, Result};
use crate::events::{EventSubscription, EventSignature};
use crate::i18n::BundleSource;
use crate::keymap::{parse_bindings, KeyBinding};
use crate::manifest::ModuleManifest;
use crate::module::Module;
use crate::rpc::MethodContribution;
use crate::runtime::Ctx;
use crate::settings::SettingSpec;
use crate::ui::{Slot, ViewContribution};
use forja_protocol::{MethodSchema, RpcError};
use serde_json::{Map, Value};
use std::any::{Any, TypeId};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Per-module registration handle. A module may only register into this.
pub struct Registrar<'a> {
    module_id: &'a str,
    registry: &'a mut Registry,
}

impl<'a> Registrar<'a> {
    pub fn module_id(&self) -> &str {
        self.module_id
    }

    /// Registers a command that is always available.
    pub fn command<F>(&mut self, id: &str, title_key: &str, handler: F)
    where
        F: for<'c, 'd> Fn(&'d mut Ctx<'c>, Args) -> Result<Value> + Send + Sync + 'static,
    {
        self.command_with(id, title_key, Availability::Always, handler);
    }

    /// Declares that this module serves a contract namespace such as `file`, so its
    /// commands and RPC methods may use ids like `file.open` (see `design/contracts.md`).
    /// Exactly one module may serve a namespace, and a namespace cannot equal a module id.
    pub fn contract_namespace(&mut self, namespace: &str) {
        self.registry
            .contract_namespaces
            .push((namespace.to_owned(), self.module_id.to_owned()));
    }

    /// Registers a command with an availability rule.
    pub fn command_with<F>(
        &mut self,
        id: &str,
        title_key: &str,
        availability: Availability,
        handler: F,
    ) where
        F: for<'c, 'd> Fn(&'d mut Ctx<'c>, Args) -> Result<Value> + Send + Sync + 'static,
    {
        self.command_with_fields(id, title_key, availability, Vec::new(), handler);
    }

    /// Registers an always-available command that asks for these values in the
    /// Anvil (as a filterable list or a text field) instead of failing when the
    /// arguments are missing.
    pub fn command_asking<F>(
        &mut self,
        id: &str,
        title_key: &str,
        fields: Vec<CommandField>,
        handler: F,
    ) where
        F: for<'c, 'd> Fn(&'d mut Ctx<'c>, Args) -> Result<Value> + Send + Sync + 'static,
    {
        self.command_with_fields(id, title_key, Availability::Always, fields, handler);
    }

    /// Registers a command with an availability rule and the values the Anvil
    /// should ask for when it runs without them.
    pub fn command_with_fields<F>(
        &mut self,
        id: &str,
        title_key: &str,
        availability: Availability,
        fields: Vec<CommandField>,
        handler: F,
    ) where
        F: for<'c, 'd> Fn(&'d mut Ctx<'c>, Args) -> Result<Value> + Send + Sync + 'static,
    {
        self.registry.commands.push(CommandSpec {
            id: CommandId::new(id),
            module: self.module_id.to_owned(),
            title_key: title_key.to_owned(),
            availability,
            fields,
            handler: Arc::new(handler),
        });
    }

    /// Registers a view whose value is already type-erased.
    pub fn view_any<F>(
        &mut self,
        slot: Slot,
        id: &str,
        title_key: &str,
        order: i32,
        factory: F,
    ) where
        F: Fn() -> Box<dyn Any> + Send + Sync + 'static,
    {
        self.registry.views.push(ViewContribution::new(
            self.module_id,
            slot,
            id,
            title_key,
            order,
            Arc::new(factory),
        ));
    }

    /// Registers a typed egui view (requires the `egui` feature).
    #[cfg(feature = "egui")]
    pub fn view<F>(&mut self, slot: Slot, id: &str, title_key: &str, order: i32, make: F)
    where
        F: Fn() -> Box<dyn crate::ui_contract::View> + Send + Sync + 'static,
    {
        self.view_any(slot, id, title_key, order, crate::ui_contract::factory(make));
    }

    /// Sets the rail glyph on the view registered with `id`.
    ///
    /// A [`Slot::Rail`] view is a full page: the shell draws this glyph (or a PNG
    /// from [`Self::view_png`]) on the rail and calls `ui()` in the center.
    pub fn view_glyph(&mut self, id: &str, glyph: &str) {
        if let Some(view) = self.registry.views.iter_mut().rev().find(|view| view.id == id) {
            view.icon_glyph = Some(glyph.to_owned());
        }
    }

    /// Sets PNG bytes as the rail icon for the view registered with `id`.
    pub fn view_png(&mut self, id: &str, png: &'static [u8]) {
        if let Some(view) = self.registry.views.iter_mut().rev().find(|view| view.id == id) {
            view.icon_png = Some(Arc::<[u8]>::from(png));
        }
    }

    pub fn setting(&mut self, spec: SettingSpec) {
        self.registry.settings.push(spec);
    }

    /// Parses the module's embedded `keymap.toml`.
    pub fn keymap(&mut self, text: &str) {
        match parse_bindings(text) {
            Ok(bindings) => self.registry.keybinds.extend(bindings),
            Err(error) => self
                .registry
                .errors
                .push(format!("module `{}` keymap.toml: {error}", self.module_id)),
        }
    }

    pub fn keybinding(&mut self, binding: KeyBinding) {
        self.registry.keybinds.push(binding);
    }

    /// Declares this module's own event type once. The name must start with the module id.
    pub fn event<T: 'static>(&mut self, name: &str) {
        self.registry.event_signatures.push(EventSignature {
            module: self.module_id.to_owned(),
            name: name.to_owned(),
            type_id: TypeId::of::<T>(),
            shared: false,
        });
    }

    /// Declares a cross-module contract event (a type from `forja-contracts`, named in
    /// `forja_contracts::names`). Publishers and subscribers both call this; repeated
    /// declarations with the same name and type are merged, and no prefix is required.
    pub fn contract_event<T: 'static>(&mut self, name: &str) {
        let type_id = TypeId::of::<T>();
        if self
            .registry
            .event_signatures
            .iter()
            .any(|signature| signature.shared && signature.type_id == type_id && signature.name == name)
        {
            return;
        }
        self.registry.event_signatures.push(EventSignature {
            module: self.module_id.to_owned(),
            name: name.to_owned(),
            type_id,
            shared: true,
        });
    }

    /// Subscribes to an event type. The type must be declared by some module (with
    /// [`Self::event`] or [`Self::contract_event`]); this is checked when the registry is
    /// finished, so registration order between publisher and subscriber does not matter.
    pub fn subscribe<T, F>(&mut self, handler: F)
    where
        T: 'static,
        F: for<'c, 'd> Fn(&'d mut Ctx<'c>, &T) + Send + Sync + 'static,
    {
        let type_id = TypeId::of::<T>();
        let adapter: crate::events::EventHandler = Arc::new(move |cx, any| {
            if let Some(event) = any.downcast_ref::<T>() {
                handler(cx, event);
            }
        });
        self.registry.subscriptions.push(EventSubscription {
            module: self.module_id.to_owned(),
            // Resolved from the declared signature in `finish`.
            name: String::new(),
            type_id,
            handler: adapter,
        });
    }

    pub fn rpc<F>(&mut self, schema: MethodSchema, handler: F)
    where
        F: for<'c, 'd> Fn(
                &'d mut Ctx<'c>,
                Map<String, Value>,
            ) -> std::result::Result<Value, RpcError>
            + Send
            + Sync
            + 'static,
    {
        self.registry.methods.push(MethodContribution {
            module: self.module_id.to_owned(),
            schema,
            handler: Arc::new(handler),
        });
    }

    /// Embeds the two Fluent bundles of this module. Call once per module.
    pub fn i18n(&mut self, en: &str, pt_br: &str) {
        if self
            .registry
            .bundles
            .iter()
            .any(|bundle| bundle.module == self.module_id)
        {
            self.registry.errors.push(format!(
                "module `{}` registered i18n bundles twice",
                self.module_id
            ));
            return;
        }
        self.registry.bundles.push(BundleSource {
            module: self.module_id.to_owned(),
            en: en.to_owned(),
            pt_br: pt_br.to_owned(),
        });
    }

    /// Announces an i18n key used at runtime (inside a handler or view), so
    /// contract tests can verify both locales contain it.
    pub fn uses_i18n(&mut self, key: &str) {
        self.registry.used_keys.insert(key.to_owned());
    }
}

/// Everything modules registered, validated and ready for the runtime.
pub struct Registry {
    modules: Vec<ModuleManifest>,
    commands: Vec<CommandSpec>,
    views: Vec<ViewContribution>,
    settings: Vec<SettingSpec>,
    keybinds: Vec<KeyBinding>,
    methods: Vec<MethodContribution>,
    event_signatures: Vec<EventSignature>,
    /// Contract namespaces (e.g. `file`) and the single module that serves each.
    contract_namespaces: Vec<(String, String)>,
    subscriptions: Vec<EventSubscription>,
    bundles: Vec<BundleSource>,
    used_keys: BTreeSet<String>,
    errors: Vec<String>,
}

impl Registry {
    fn new(modules: Vec<ModuleManifest>) -> Self {
        Self {
            modules,
            commands: Vec::new(),
            views: Vec::new(),
            settings: Vec::new(),
            keybinds: Vec::new(),
            methods: Vec::new(),
            event_signatures: Vec::new(),
            contract_namespaces: Vec::new(),
            subscriptions: Vec::new(),
            bundles: Vec::new(),
            used_keys: BTreeSet::new(),
            errors: Vec::new(),
        }
    }

    pub fn modules(&self) -> &[ModuleManifest] {
        &self.modules
    }

    pub fn commands(&self) -> &[CommandSpec] {
        &self.commands
    }

    pub fn command(&self, id: &str) -> Option<&CommandSpec> {
        self.commands.iter().find(|command| command.id.as_str() == id)
    }

    pub fn views(&self) -> &[ViewContribution] {
        &self.views
    }

    pub fn views_in(&self, slot: Slot) -> Vec<&ViewContribution> {
        let mut views: Vec<&ViewContribution> = self
            .views
            .iter()
            .filter(|view| view.slot == slot)
            .collect();
        views.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
        views
    }

    pub fn settings(&self) -> &[SettingSpec] {
        &self.settings
    }

    pub fn keybinds(&self) -> &[KeyBinding] {
        &self.keybinds
    }

    /// Conflicts among module defaults (user/platform layers are checked by
    /// the runtime after loading those files).
    pub fn keymap_conflicts(&self) -> Vec<crate::keymap::KeyConflict> {
        crate::keymap::Keymap::build(self.keybinds.clone(), Vec::new())
            .0
            .conflicts()
            .to_vec()
    }

    pub fn methods(&self) -> &[MethodContribution] {
        &self.methods
    }

    pub fn method(&self, name: &str) -> Option<&MethodContribution> {
        self.methods.iter().find(|method| method.schema.name == name)
    }

    pub fn event_signatures(&self) -> &[EventSignature] {
        &self.event_signatures
    }

    pub fn subscriptions(&self) -> &[EventSubscription] {
        &self.subscriptions
    }

    pub fn bundles(&self) -> &[BundleSource] {
        &self.bundles
    }

    /// Every i18n key the registry knows about, sorted.
    pub fn i18n_keys(&self) -> Vec<String> {
        let mut keys: BTreeSet<String> = self.used_keys.clone();
        for module in &self.modules {
            keys.insert(module.name_key.clone());
        }
        for command in &self.commands {
            keys.insert(command.title_key.clone());
        }
        for view in &self.views {
            keys.insert(view.title_key.clone());
        }
        for setting in &self.settings {
            keys.insert(setting.label_key.clone());
            keys.insert(setting.help_key.clone());
        }
        for method in &self.methods {
            keys.insert(method.schema.description_key.clone());
            if let Some(result) = &method.schema.result {
                keys.insert(result.description_key.clone());
            }
        }
        for binding in &self.keybinds {
            if let Some(title_key) = &binding.title_key {
                keys.insert(title_key.clone());
            }
        }
        keys.into_iter().collect()
    }

    fn finalize(mut self) -> Result<Self> {
        let mut errors = std::mem::take(&mut self.errors);
        let mut module_ids = BTreeSet::new();
        for module in &self.modules {
            if module.id.trim().is_empty() {
                errors.push("a module has an empty id".to_owned());
            }
            if !module_ids.insert(module.id.clone()) {
                errors.push(format!("duplicate module id `{}`", module.id));
            }
        }

        let command_ids: BTreeSet<String> = self
            .commands
            .iter()
            .map(|command| command.id.as_str().to_owned())
            .collect();
        let mut namespace_owner: std::collections::BTreeMap<&str, &str> = Default::default();
        for (namespace, module) in &self.contract_namespaces {
            if module_ids.contains(namespace) {
                errors.push(format!(
                    "contract namespace `{namespace}` clashes with the module id `{namespace}`"
                ));
            }
            match namespace_owner.insert(namespace.as_str(), module.as_str()) {
                Some(other) if other != module => errors.push(format!(
                    "contract namespace `{namespace}` is served by both `{other}` and `{module}`"
                )),
                _ => {}
            }
        }
        let owns = |id: &str, module: &str| {
            namespace_owner.iter().any(|(namespace, owner)| {
                *owner == module
                    && id
                        .strip_prefix(namespace)
                        .is_some_and(|rest| rest.starts_with('.') && rest.len() > 1)
            })
        };
        let mut seen = BTreeSet::new();
        for command in &self.commands {
            if !seen.insert(command.id.as_str().to_owned()) {
                errors.push(format!("duplicate command id `{}`", command.id));
            }
            if !owns(command.id.as_str(), &command.module) {
                require_prefix("command", command.id.as_str(), &command.module, &mut errors);
            }
        }
        let mut seen = BTreeSet::new();
        for method in &self.methods {
            let name = method.schema.name.as_str();
            if !seen.insert(name.to_owned()) {
                errors.push(format!("duplicate RPC method `{name}`"));
            }
            if !owns(name, &method.module) {
                require_prefix("RPC method", name, &method.module, &mut errors);
            }
            let mut params = BTreeSet::new();
            for param in &method.schema.params {
                if !params.insert(param.name.clone()) {
                    errors.push(format!("RPC method `{name}` repeats parameter `{}`", param.name));
                }
            }
        }
        let mut seen = BTreeSet::new();
        for view in &self.views {
            if !seen.insert((view.slot, view.id.clone())) {
                errors.push(format!(
                    "duplicate view `{}` in slot `{}`",
                    view.id,
                    view.slot.id()
                ));
            }
            require_prefix("view", &view.id, &view.module, &mut errors);
        }
        let mut seen = BTreeSet::new();
        for setting in &self.settings {
            if !seen.insert(setting.key.clone()) {
                errors.push(format!("duplicate setting `{}`", setting.key));
            }
            if let Err(reason) = setting.validate() {
                errors.push(reason);
            }
            if !module_ids.contains(setting.module()) {
                errors.push(format!(
                    "setting `{}` belongs to unknown module `{}`",
                    setting.key,
                    setting.module()
                ));
            }
        }
        let mut types = BTreeSet::new();
        let mut names = BTreeSet::new();
        for signature in &self.event_signatures {
            if !types.insert(signature.type_id) {
                errors.push(format!(
                    "event type declared twice (`{}`)",
                    signature.name
                ));
            }
            if !names.insert(signature.name.clone()) {
                errors.push(format!("duplicate event name `{}`", signature.name));
            }
            if !signature.shared {
                require_prefix("event", &signature.name, &signature.module, &mut errors);
            }
        }
        for subscription in &mut self.subscriptions {
            match self
                .event_signatures
                .iter()
                .find(|signature| signature.type_id == subscription.type_id)
            {
                Some(signature) => subscription.name = signature.name.clone(),
                None => errors.push(format!(
                    "module `{}` subscribed to an event type nobody declared",
                    subscription.module
                )),
            }
        }
        let modules: BTreeSet<&str> = module_ids.iter().map(String::as_str).collect();
        for binding in &self.keybinds {
            if !command_ids.contains(binding.command.as_str()) {
                errors.push(format!(
                    "key binding points to unknown command `{}`",
                    binding.command
                ));
            }
        }
        for signature in &self.event_signatures {
            if !modules.contains(signature.module.as_str()) {
                errors.push(format!(
                    "event `{}` belongs to unknown module `{}`",
                    signature.name, signature.module
                ));
            }
        }
        if errors.is_empty() {
            Ok(self)
        } else {
            Err(KernelError::Registration(errors.join("; ")))
        }
    }
}

fn require_prefix(kind: &str, id: &str, module: &str, errors: &mut Vec<String>) {
    let matches = id
        .strip_prefix(module)
        .is_some_and(|rest| rest.starts_with('.') && rest.len() > 1);
    if !matches {
        errors.push(format!("{kind} `{id}` must start with `{module}.`"));
    }
}

/// Instantiates the modules, orders them by manifest dependencies and runs
/// registration.
pub fn build_registry(modules: Vec<Box<dyn Module>>) -> Result<Registry> {
    let manifests: Vec<ModuleManifest> = modules.iter().map(|module| module.manifest()).collect();
    let order = order_modules(&manifests)?;
    let mut ordered_manifests = Vec::with_capacity(manifests.len());
    let mut slots: Vec<Option<Box<dyn Module>>> = modules.into_iter().map(Some).collect();
    let mut ordered_modules = Vec::with_capacity(manifests.len());
    for index in order {
        ordered_manifests.push(manifests[index].clone());
        if let Some(module) = slots[index].take() {
            ordered_modules.push(module);
        }
    }
    let mut registry = Registry::new(ordered_manifests);
    for module in &mut ordered_modules {
        let module_id = module.manifest().id;
        let mut registrar = Registrar {
            module_id: &module_id,
            registry: &mut registry,
        };
        module.register(&mut registrar);
    }
    registry.finalize()
}

fn order_modules(manifests: &[ModuleManifest]) -> Result<Vec<usize>> {
    let mut ids: BTreeMap<&str, usize> = BTreeMap::new();
    for (index, manifest) in manifests.iter().enumerate() {
        if ids.insert(manifest.id.as_str(), index).is_some() {
            return Err(KernelError::registration(format!(
                "duplicate module id `{}`",
                manifest.id
            )));
        }
    }
    let mut indegree = vec![0usize; manifests.len()];
    let mut dependents: Vec<Vec<usize>> = vec![Vec::new(); manifests.len()];
    for (index, manifest) in manifests.iter().enumerate() {
        for dependency in &manifest.depends_on {
            let Some(&dependency_index) = ids.get(dependency.as_str()) else {
                return Err(KernelError::registration(format!(
                    "module `{}` depends on unknown module `{dependency}`",
                    manifest.id
                )));
            };
            indegree[index] += 1;
            dependents[dependency_index].push(index);
        }
    }
    let mut ready: BTreeSet<(String, usize)> = manifests
        .iter()
        .enumerate()
        .filter(|(index, _)| indegree[*index] == 0)
        .map(|(index, manifest)| (manifest.id.clone(), index))
        .collect();
    let mut order = Vec::with_capacity(manifests.len());
    while let Some((_, index)) = ready.pop_first() {
        order.push(index);
        for &dependent in &dependents[index] {
            indegree[dependent] -= 1;
            if indegree[dependent] == 0 {
                ready.insert((manifests[dependent].id.clone(), dependent));
            }
        }
    }
    if order.len() != manifests.len() {
        let cycle: Vec<&str> = manifests
            .iter()
            .enumerate()
            .filter(|(index, _)| indegree[*index] > 0)
            .map(|(_, manifest)| manifest.id.as_str())
            .collect();
        return Err(KernelError::registration(format!(
            "module dependency cycle among: {}",
            cycle.join(", ")
        )));
    }
    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::ModuleManifest;
    use crate::module::Module;

    struct TestModule {
        manifest: ModuleManifest,
        register: fn(&mut Registrar<'_>),
    }

    impl Module for TestModule {
        fn manifest(&self) -> ModuleManifest {
            self.manifest.clone()
        }

        fn register(&mut self, registrar: &mut Registrar<'_>) {
            (self.register)(registrar);
        }
    }

    fn module(id: &str, depends_on: &[&str], register: fn(&mut Registrar<'_>)) -> Box<dyn Module> {
        Box::new(TestModule {
            manifest: ModuleManifest::new(id, format!("{id}.name"), "0.1.0")
                .depends_on(depends_on.iter().copied()),
            register,
        })
    }

    fn empty(_: &mut Registrar<'_>) {}

    fn duplicate_command(registrar: &mut Registrar<'_>) {
        registrar.command("alpha.one", "alpha.one.title", |_, _| Ok(Value::Null));
        registrar.command("alpha.one", "alpha.one.title", |_, _| Ok(Value::Null));
    }

    struct SharedEvent;
    struct OwnEvent;

    fn subscriber_first(registrar: &mut Registrar<'_>) {
        registrar.contract_event::<SharedEvent>("bench.activated");
        registrar.subscribe(|_, _: &SharedEvent| {});
    }

    fn publisher_later(registrar: &mut Registrar<'_>) {
        registrar.contract_event::<SharedEvent>("bench.activated");
    }

    fn own_event_wrong_prefix(registrar: &mut Registrar<'_>) {
        registrar.event::<OwnEvent>("other.thing");
    }

    fn subscribe_undeclared(registrar: &mut Registrar<'_>) {
        registrar.subscribe(|_, _: &OwnEvent| {});
    }

    fn serves_file(registrar: &mut Registrar<'_>) {
        registrar.contract_namespace("file");
        registrar.command("file.open", "editor.command.open", |_, _| Ok(Value::Null));
    }

    fn also_serves_file(registrar: &mut Registrar<'_>) {
        registrar.contract_namespace("file");
    }

    fn command_outside_namespace(registrar: &mut Registrar<'_>) {
        registrar.command("file.open", "x.command.open", |_, _| Ok(Value::Null));
    }

    #[test]
    fn contract_namespaces_allow_role_ids_for_exactly_one_module() {
        assert!(build_registry(vec![module("editor", &[], serves_file)]).is_ok());
        let error = build_registry(vec![
            module("editor", &[], serves_file),
            module("other", &[], also_serves_file),
        ])
        .err()
        .expect("two owners fail");
        assert!(error.to_string().contains("served by both"));
        let error = build_registry(vec![module("viewer", &[], command_outside_namespace)])
            .err()
            .expect("no namespace, no role id");
        assert!(error.to_string().contains("must start with `viewer.`"));
    }

    #[test]
    fn contract_events_merge_across_modules_in_any_order() {
        let modules = vec![
            module("explorer", &[], subscriber_first),
            module("bench", &[], publisher_later),
        ];
        let registry = build_registry(modules).expect("contract events register");
        assert_eq!(registry.event_signatures().len(), 1);
        assert_eq!(registry.subscriptions()[0].name, "bench.activated");
    }

    #[test]
    fn own_events_keep_the_prefix_rule_and_subscriptions_need_a_declaration() {
        let error = build_registry(vec![module("alpha", &[], own_event_wrong_prefix)])
            .err()
            .expect("prefix enforced");
        assert!(error.to_string().contains("must start with `alpha.`"));
        let error = build_registry(vec![module("alpha", &[], subscribe_undeclared)])
            .err()
            .expect("undeclared subscription fails");
        assert!(error.to_string().contains("nobody declared"));
    }

    #[test]
    fn dependencies_are_ordered_before_dependents() {
        let modules = vec![
            module("beta", &["alpha"], empty),
            module("alpha", &[], empty),
        ];
        let registry = build_registry(modules).unwrap();
        let ids: Vec<&str> = registry.modules().iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "beta"]);
    }

    #[test]
    fn cycles_and_unknown_dependencies_fail() {
        let cycle = vec![module("a", &["b"], empty), module("b", &["a"], empty)];
        assert!(build_registry(cycle).is_err());
        let missing = vec![module("a", &["nope"], empty)];
        assert!(build_registry(missing).is_err());
    }

    #[test]
    fn duplicate_ids_are_registration_errors() {
        let modules = vec![module("alpha", &[], duplicate_command)];
        let error = build_registry(modules).err().expect("duplicate fails");
        assert!(error.to_string().contains("duplicate command"));
    }
}
