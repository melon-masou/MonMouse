use std::ops::RangeInclusive;

pub enum Layout<'a> {
    Toolbar,
    Row,
    Card,
    Form {
        enabled: bool,
    },
    Section {
        title: &'a str,
    },
    Field {
        label: &'a str,
        hint: Option<&'a str>,
    },
}

pub enum Input<'a> {
    Text {
        value: &'a mut String,
        max_chars: Option<usize>,
        placeholder: &'a str,
    },
    Unsigned {
        value: &'a mut u64,
        range: RangeInclusive<u64>,
    },
    Bool(&'a mut bool),
    Choice {
        selected: &'a mut usize,
        labels: &'a [&'a str],
    },
    Shortcut(&'a mut String),
}

pub enum StatusLight {
    Off,
    On,
    Error,
}

pub type Draw<'a> = dyn FnMut(&mut PluginUi<'_>) + 'a;

pub trait UiBackend {
    fn language(&self) -> &str;
    fn layout(&mut self, layout: Layout<'_>, body: &mut Draw<'_>);
    fn collapsible_card(
        &mut self,
        title: &str,
        expanded: bool,
        actions: &mut Draw<'_>,
        body: &mut Draw<'_>,
    ) -> bool;
    fn heading(&mut self, text: &str);
    fn text(&mut self, text: &str);
    fn status(&mut self, label: &str, light: StatusLight, detail: &str);
    fn icon_button(&mut self, icon: &str, hint: &str) -> bool;
    fn edit(&mut self, input: Input<'_>, width_chars: Option<usize>) -> bool;
    fn button(&mut self, label: &str, enabled: bool) -> bool;
    fn request_discard(&mut self);
}

pub struct PluginUi<'a> {
    backend: &'a mut dyn UiBackend,
}

impl<'a> PluginUi<'a> {
    pub fn new(backend: &'a mut dyn UiBackend) -> Self {
        Self { backend }
    }

    pub fn language(&self) -> &str {
        self.backend.language()
    }

    pub fn heading(&mut self, text: &str) {
        self.backend.heading(text);
    }

    pub fn text(&mut self, text: &str) {
        self.backend.text(text);
    }

    pub fn status(&mut self, label: &str, light: StatusLight, detail: &str) {
        self.backend.status(label, light, detail);
    }

    pub fn icon_button(&mut self, icon: &str, hint: &str) -> bool {
        self.backend.icon_button(icon, hint)
    }

    pub fn collapsible_card(
        &mut self,
        title: &str,
        expanded: bool,
        mut actions: impl FnMut(&mut PluginUi<'_>),
        mut body: impl FnMut(&mut PluginUi<'_>),
    ) -> bool {
        self.backend
            .collapsible_card(title, expanded, &mut actions, &mut body)
    }

    pub fn button(&mut self, label: &str, enabled: bool) -> bool {
        self.backend.button(label, enabled)
    }

    pub fn request_discard(&mut self) {
        self.backend.request_discard();
    }

    pub fn toolbar(&mut self, mut body: impl FnMut(&mut PluginUi<'_>)) {
        self.backend.layout(Layout::Toolbar, &mut body);
    }

    pub fn row(&mut self, mut body: impl FnMut(&mut PluginUi<'_>)) {
        self.backend.layout(Layout::Row, &mut body);
    }

    pub fn card(&mut self, mut body: impl FnMut(&mut PluginUi<'_>)) {
        self.backend.layout(Layout::Card, &mut body);
    }

    pub fn form(&mut self, enabled: bool, mut body: impl FnMut(&mut PluginUi<'_>)) {
        self.backend.layout(Layout::Form { enabled }, &mut body);
    }

    pub fn section(&mut self, title: &str, mut body: impl FnMut(&mut PluginUi<'_>)) {
        self.backend.layout(Layout::Section { title }, &mut body);
    }

    pub fn field(
        &mut self,
        label: &str,
        hint: Option<&str>,
        mut body: impl FnMut(&mut PluginUi<'_>),
    ) {
        self.backend
            .layout(Layout::Field { label, hint }, &mut body);
    }

    pub fn edit(&mut self, input: Input<'_>, width_chars: Option<usize>) -> bool {
        self.backend.edit(input, width_chars)
    }

    pub fn edit_text(&mut self, value: &mut String, max_chars: Option<usize>) -> bool {
        self.edit_text_with_placeholder(value, max_chars, "")
    }

    pub fn edit_text_with_placeholder(
        &mut self,
        value: &mut String,
        max_chars: Option<usize>,
        placeholder: &str,
    ) -> bool {
        self.edit(
            Input::Text {
                value,
                max_chars,
                placeholder,
            },
            None,
        )
    }

    pub fn edit_unsigned(&mut self, value: &mut u64, range: RangeInclusive<u64>) -> bool {
        self.edit(Input::Unsigned { value, range }, None)
    }

    pub fn edit_bool(&mut self, value: &mut bool) -> bool {
        self.edit(Input::Bool(value), None)
    }

    pub fn edit_choice(&mut self, selected: &mut usize, labels: &[&str]) -> bool {
        self.edit(Input::Choice { selected, labels }, None)
    }

    pub fn edit_shortcut(&mut self, value: &mut String) -> bool {
        self.edit(Input::Shortcut(value), None)
    }

    pub fn btn(&mut self, label: &str) -> bool {
        self.button(label, true)
    }
}
