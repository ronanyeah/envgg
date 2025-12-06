use crate::{add_secret_to_keyring, delete_secret_from_keyring, get_secret_from_keyring};
use gpui::{
    App, AppContext, Bounds, Context, Entity, FocusHandle, Focusable, InteractiveElement,
    IntoElement, ParentElement, Render, SharedString, Size, Styled, Window, WindowBounds,
    WindowOptions, div, px, size,
};
use gpui_component::{
    ActiveTheme, IconName, Root, Sizable, StyledExt, Theme, ThemeMode, TitleBar, WindowExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    label::Label,
    list::{List, ListDelegate, ListItem, ListState},
    v_flex,
};
use gpui_component_assets::Assets;
use std::rc::Rc;

#[derive(IntoElement)]
struct SecretListItem {
    base: ListItem,
    secret: Rc<SharedString>,
    on_delete: Option<Box<dyn Fn(&str, &mut Window, &mut App) + 'static>>,
    on_copy: Option<Box<dyn Fn(&str, &mut Window, &mut App) + 'static>>,
}

impl SecretListItem {
    pub fn new(id: impl Into<gpui::ElementId>, secret: Rc<SharedString>) -> Self {
        SecretListItem {
            secret,
            base: ListItem::new(id),
            on_delete: None,
            on_copy: None,
        }
    }

    pub fn on_delete(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_delete = Some(Box::new(handler));
        self
    }

    pub fn on_copy(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_copy = Some(Box::new(handler));
        self
    }
}

impl gpui_component::Selectable for SecretListItem {
    fn selected(self, _selected: bool) -> Self {
        self
    }

    fn is_selected(&self) -> bool {
        false
    }
}

impl gpui::RenderOnce for SecretListItem {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let text_color = cx.theme().foreground;

        let secret_name = self.secret.to_string();
        let secret_name_copy = secret_name.clone();
        let secret_name_delete = secret_name.clone();
        let on_delete = self.on_delete;
        let on_copy = self.on_copy;

        self.base
            .px_2()
            .py_1()
            .overflow_x_hidden()
            .border_1()
            .rounded(cx.theme().radius)
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .text_color(text_color)
                    .child(
                        h_flex().gap_2().child(
                            v_flex()
                                .gap_1()
                                .max_w(px(500.))
                                .overflow_x_hidden()
                                .flex_nowrap()
                                .child(Label::new(secret_name.clone()).whitespace_nowrap()),
                        ),
                    )
                    .child(
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new(SharedString::from(format!(
                                    "copy-{}",
                                    secret_name_copy
                                )))
                                .icon(IconName::Copy)
                                .small()
                                .on_click(
                                    move |_, window, cx| {
                                        if let Some(ref handler) = on_copy {
                                            handler(&secret_name_copy, window, cx);
                                        }
                                    },
                                ),
                            )
                            .child(
                                Button::new(SharedString::from(format!(
                                    "delete-{}",
                                    secret_name_delete
                                )))
                                .icon(IconName::Close)
                                .small()
                                .on_click(
                                    move |_, window, cx| {
                                        if let Some(ref handler) = on_delete {
                                            handler(&secret_name_delete, window, cx);
                                        }
                                    },
                                ),
                            ),
                    ),
            )
    }
}

struct SecretListDelegate {
    secrets: Vec<Rc<SharedString>>,
    filtered_secrets: Vec<Rc<SharedString>>,
    query: SharedString,
    on_delete: Option<Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>>,
    on_copy: Option<Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>>,
}

impl SecretListDelegate {
    fn new(secrets: Vec<String>) -> Self {
        let secrets: Vec<Rc<_>> = secrets
            .into_iter()
            .map(|name| Rc::new(SharedString::new(name)))
            .collect();
        let filtered_secrets = secrets.clone();

        Self {
            secrets,
            filtered_secrets,
            query: "".into(),
            on_delete: None,
            on_copy: None,
        }
    }

    fn set_on_delete(&mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) {
        self.on_delete = Some(Rc::new(handler));
    }

    fn set_on_copy(&mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) {
        self.on_copy = Some(Rc::new(handler));
    }

    fn filter(&mut self, query: impl Into<SharedString>) {
        self.query = query.into();
        self.filtered_secrets = self
            .secrets
            .iter()
            .filter(|secret| secret.to_lowercase().contains(&self.query.to_lowercase()))
            .cloned()
            .collect();
    }
}

impl ListDelegate for SecretListDelegate {
    type Item = SecretListItem;

    fn sections_count(&self, _: &App) -> usize {
        1
    }

    fn items_count(&self, _section: usize, _: &App) -> usize {
        self.filtered_secrets.len()
    }

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> gpui::Task<()> {
        self.filter(query.to_owned());
        gpui::Task::ready(())
    }

    fn set_selected_index(
        &mut self,
        _ix: Option<gpui_component::IndexPath>,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) {
        cx.notify();
    }

    fn render_item(
        &self,
        ix: gpui_component::IndexPath,
        _: &mut Window,
        _: &mut App,
    ) -> Option<Self::Item> {
        if let Some(secret) = self.filtered_secrets.get(ix.row) {
            let mut item = SecretListItem::new(ix, secret.clone());
            if let Some(ref on_delete) = self.on_delete {
                let on_delete_clone = on_delete.clone();
                item = item.on_delete(move |name, window, cx| {
                    on_delete_clone(name, window, cx);
                });
            }
            if let Some(ref on_copy) = self.on_copy {
                let on_copy_clone = on_copy.clone();
                item = item.on_copy(move |name, window, cx| {
                    on_copy_clone(name, window, cx);
                });
            }
            return Some(item);
        }

        None
    }

    fn loading(&self, _: &App) -> bool {
        false
    }

    fn is_eof(&self, _: &App) -> bool {
        true
    }
}

pub struct SecretsViewer {
    focus_handle: FocusHandle,
    secrets_list: Entity<ListState<SecretListDelegate>>,
}

impl SecretsViewer {
    pub fn new(secrets: Vec<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let delegate = SecretListDelegate::new(secrets);
        let secrets_list = cx.new(|cx| ListState::new(delegate, window, cx).searchable(true));

        Self {
            focus_handle: cx.focus_handle(),
            secrets_list,
        }
    }

    pub fn view(secrets: Vec<String>, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let view = cx.new(|cx| Self::new(secrets, window, cx));
        view.update(cx, |this, cx| {
            this.setup_delete_handler(view.clone(), cx);
            this.setup_copy_handler(view.clone(), cx);
        });
        view
    }

    fn setup_delete_handler(&mut self, view: Entity<Self>, cx: &mut Context<Self>) {
        self.secrets_list.update(cx, |list, _| {
            list.delegate_mut().set_on_delete(move |name, window, cx| {
                view.update(cx, |this, cx| {
                    this.show_delete_confirmation(name, window, cx, view.clone());
                });
            });
        });
    }

    fn setup_copy_handler(&mut self, view: Entity<Self>, cx: &mut Context<Self>) {
        self.secrets_list.update(cx, |list, _| {
            list.delegate_mut().set_on_copy(move |name, window, cx| {
                view.update(cx, |this, cx| {
                    this.handle_copy_secret(name.to_string(), window, cx);
                });
            });
        });
    }

    fn show_delete_confirmation(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut App,
        view: Entity<Self>,
    ) {
        let name = name.to_string();

        window.open_dialog(cx, move |dialog, _, _| {
            let name = name.clone();
            let view = view.clone();

            dialog
                .confirm()
                .overlay(true)
                .overlay_closable(true)
                .child(format!(
                    "Are you sure you want to delete the secret '{}'?",
                    name
                ))
                .on_ok(move |_, window, cx| {
                    view.update(cx, |this, cx| {
                        this.handle_delete_secret(name.clone(), window, cx);
                    });
                    true
                })
                .on_cancel(|_, _, _| true)
        });
    }

    fn handle_delete_secret(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let task =
            cx.spawn_in(
                window,
                async move |view_entity, window| match delete_secret_from_keyring(&name).await {
                    Ok(_) => {
                        Self::refresh_secrets_with_notification(
                            view_entity,
                            window,
                            name,
                            "deleted",
                        )
                        .await;
                    }
                    Err(e) => {
                        Self::show_error_notification(
                            view_entity,
                            window,
                            format!("Error deleting secret: {}", e),
                        )
                        .await;
                    }
                },
            );
        task.detach();
    }

    fn handle_copy_secret(&mut self, name: String, window: &mut Window, cx: &mut Context<Self>) {
        let task =
            cx.spawn_in(
                window,
                async move |view_entity, window| match get_secret_from_keyring(&name).await {
                    Ok(value) => {
                        _ = view_entity.update_in(window, move |_, window, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(value));
                            window.push_notification(
                                format!("Secret '{}' copied to clipboard", name),
                                cx,
                            );
                        });
                    }
                    Err(e) => {
                        Self::show_error_notification(
                            view_entity,
                            window,
                            format!("Error copying secret: {}", e),
                        )
                        .await;
                    }
                },
            );
        task.detach();
    }

    async fn refresh_secrets_with_notification(
        view_entity: gpui::WeakEntity<Self>,
        window: &mut gpui::AsyncWindowContext,
        secret_name: String,
        operation: &str,
    ) {
        match crate::list_secrets().await {
            Ok(secrets) => {
                let view_weak = view_entity.clone();
                _ = view_entity.update_in(window, move |view_ref, window, cx| {
                    view_ref.refresh_delegate_with_secrets(secrets, view_weak.clone(), cx);
                    window.push_notification(
                        format!("Secret '{}' {} successfully", secret_name, operation),
                        cx,
                    );
                });
            }
            Err(e) => {
                Self::show_error_notification(
                    view_entity,
                    window,
                    format!("Error refreshing secrets: {}", e),
                )
                .await;
            }
        }
    }

    fn refresh_delegate_with_secrets(
        &mut self,
        secrets: Vec<String>,
        view_weak: gpui::WeakEntity<Self>,
        cx: &mut Context<Self>,
    ) {
        self.secrets_list.update(cx, |list, cx| {
            let mut delegate = SecretListDelegate::new(secrets);
            let view_weak_delete = view_weak.clone();
            delegate.set_on_delete(move |name, window, cx| {
                if let Some(v) = view_weak_delete.upgrade() {
                    v.update(cx, |this, cx| {
                        this.show_delete_confirmation(name, window, cx, v.clone());
                    });
                }
            });
            delegate.set_on_copy(move |name, window, cx| {
                if let Some(v) = view_weak.upgrade() {
                    v.update(cx, |this, cx| {
                        this.handle_copy_secret(name.to_string(), window, cx);
                    });
                }
            });
            *list.delegate_mut() = delegate;
            cx.notify();
        });
    }

    async fn show_error_notification(
        view_entity: gpui::WeakEntity<Self>,
        window: &mut gpui::AsyncWindowContext,
        message: String,
    ) {
        _ = view_entity.update_in(window, move |_, window, cx| {
            window.push_notification(message, cx);
        });
    }

    fn open_add_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = cx.entity().clone();
        let new_key_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Key name (e.g., API_KEY)"));
        let new_value_input = cx.new(|cx| InputState::new(window, cx).placeholder("Secret value"));

        window.open_dialog(cx, move |dialog, _window, _cx| {
            let view = view.clone();
            let key_input = new_key_input.clone();
            let value_input = new_value_input.clone();

            let form_content = v_flex()
                .gap_4()
                .child(
                    v_flex()
                        .gap_2()
                        .child(Label::new("Key Name"))
                        .child(Input::new(&new_key_input)),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(Label::new("Secret Value"))
                        .child(Input::new(&new_value_input)),
                );

            dialog
                .title("Add New Secret")
                .child(form_content)
                .footer(move |_, _, _, _| {
                    Self::create_add_dialog_buttons(
                        view.clone(),
                        key_input.clone(),
                        value_input.clone(),
                    )
                })
        });
    }

    fn create_add_dialog_buttons(
        view: Entity<Self>,
        key_input: Entity<InputState>,
        value_input: Entity<InputState>,
    ) -> Vec<Button> {
        vec![
            Button::new("cancel")
                .label("Cancel")
                .on_click(move |_, window, cx| {
                    window.close_dialog(cx);
                }),
            Button::new("add")
                .label("Add Secret")
                .primary()
                .on_click(move |_, window, cx| {
                    let key = key_input.read(cx).text().to_string();
                    let value = value_input.read(cx).text().to_string();


                    if !Self::is_valid_env_var_name(&key) {
                        window.push_notification(
                            "Key must be in SCREAMING_CASE (uppercase letters, numbers, and underscores only, starting with a letter)",
                            cx,
                        );
                        return;
                    }

                    if value.is_empty() {
                        window.push_notification("Key and value cannot be empty", cx);
                        return;
                    }

                    window.close_dialog(cx);

                    view.update(cx, move |this, cx| {
                        this.handle_add_secret(key, value, window, cx);
                    });
                }),
        ]
    }

    fn is_valid_env_var_name(name: &str) -> bool {
        let mut chars = name.chars();

        // First character must be a letter (A-Z)
        if let Some(first) = chars.next() {
            if !first.is_ascii_uppercase() {
                return false;
            }
        } else {
            return false;
        }

        // Remaining characters must be uppercase letters, digits, or underscores
        for ch in chars {
            if !ch.is_ascii_uppercase() && !ch.is_ascii_digit() && ch != '_' {
                return false;
            }
        }

        // Check if it's actually in SCREAMING_CASE (contains at least one uppercase)
        name.chars().any(|c| c.is_ascii_uppercase())
    }

    fn handle_add_secret(
        &mut self,
        key: String,
        value: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let task = cx.spawn_in(
            window,
            async move |view_entity, window| match add_secret_to_keyring(&key, &value).await {
                Ok(_) => {
                    Self::refresh_secrets_with_notification(view_entity, window, key, "added")
                        .await;
                }
                Err(e) => {
                    Self::show_error_notification(
                        view_entity,
                        window,
                        format!("Error adding secret: {}", e),
                    )
                    .await;
                }
            },
        );
        task.detach();
    }
}

impl Focusable for SecretsViewer {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SecretsViewer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .track_focus(&self.focus_handle)
            .size_full()
            .gap_4()
            .p_4()
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_xl().font_bold().child("envgg"))
                    .child(
                        h_flex().gap_2().child(
                            Button::new("add-secret-btn")
                                .icon(IconName::Plus)
                                .label("Add Secret")
                                .primary()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_add_dialog(window, cx);
                                })),
                        ),
                    ),
            )
            .child(
                List::new(&self.secrets_list)
                    .p(px(8.))
                    .flex_1()
                    .w_full()
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius),
            )
    }
}

struct AppRoot {
    view: gpui::AnyView,
}

impl AppRoot {
    pub fn new(view: impl Into<gpui::AnyView>) -> Self {
        Self { view: view.into() }
    }
}

impl Render for AppRoot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        div()
            .size_full()
            .child(
                v_flex()
                    .size_full()
                    .child(TitleBar::new())
                    .child(div().flex_1().child(self.view.clone())),
            )
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}

pub async fn open_secrets_viewer() {
    let secrets = match crate::list_secrets().await {
        Ok(secrets) => secrets,
        Err(e) => {
            panic!("Error loading secrets: {}", e);
        }
    };

    let app = gpui::Application::new().with_assets(Assets);

    app.run(move |cx| {
        gpui_component::init(cx);

        cx.activate(true);

        Theme::change(ThemeMode::Dark, None, cx);

        let window_size = size(px(800.0), px(600.0));
        let window_size = if let Some(display) = cx.primary_display() {
            let display_size = display.bounds().size;
            Size {
                width: window_size.width.min(display_size.width * 0.85),
                height: window_size.height.min(display_size.height * 0.85),
            }
        } else {
            window_size
        };

        let window_bounds = Bounds::centered(None, window_size, cx);
        let title = SharedString::from("envgg");

        cx.spawn(async move |cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(window_bounds)),
                titlebar: Some(TitleBar::title_bar_options()),
                window_min_size: Some(Size {
                    width: px(480.),
                    height: px(320.),
                }),
                ..Default::default()
            };

            let window = cx.open_window(options, |window, cx| {
                let view = SecretsViewer::view(secrets, window, cx);
                let root = cx.new(|_cx| AppRoot::new(view));

                cx.new(|cx| Root::new(root, window, cx))
            })?;

            window.update(cx, |_, window, _| {
                window.activate_window();
                window.set_window_title(&title);
            })?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
