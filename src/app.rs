// SPDX-License-Identifier: {{LICENSE}}

use crate::config::TootConfig;
use crate::pages::public::TimelineType;
use crate::pages::Page;
use crate::utils::{self, Cache};
use crate::widgets::status::StatusOptions;
use crate::{fl, pages, widgets};
use cosmic::app::{context_drawer, Core, Task};
use cosmic::cosmic_config;
use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::{Length, Subscription};
use cosmic::widget::about::About;
use cosmic::widget::image::Handle;
use cosmic::widget::menu::{ItemHeight, ItemWidth};
use cosmic::widget::{self, menu, nav_bar};
use cosmic::{Application, ApplicationExt, Apply, Element};
use mastodon_async::helpers::toml;
use mastodon_async::prelude::{Account, Notification, Scopes, Status, StatusId};
use mastodon_async::registration::Registered;
use mastodon_async::{Data, Mastodon, NewStatus, Registration};
use reqwest::Url;
use std::collections::{HashMap, VecDeque};
use std::str::FromStr;

const REPOSITORY: &str = "https://github.com/edfloreshz/toot";
const SUPPORT: &str = "https://github.com/edfloreshz/toot/issues";

pub struct AppModel {
    core: Core,
    about: About,
    nav: nav_bar::Model,
    context_page: ContextPage,
    key_binds: HashMap<menu::KeyBind, MenuAction>,
    dialog_pages: VecDeque<Dialog>,
    dialog_editor: widget::text_editor::Content,
    config: TootConfig,
    handler: Option<cosmic_config::Config>,
    instance: String,
    code: String,
    registration: Option<Registered>,
    mastodon: Mastodon,
    cache: Cache,
    home: pages::home::Home,
    notifications: pages::notifications::Notifications,
    explore: pages::public::Public,
    local: pages::public::Public,
    federated: pages::public::Public,
}

#[derive(Debug, Clone)]
pub enum Message {
    Open(String),
    ToggleContextPage(ContextPage),
    ToggleContextDrawer,
    UpdateConfig(TootConfig),
    InstanceEdit,
    RegisterMastodonClient,
    CompleteRegistration,
    StoreMastodonData(Mastodon),
    StoreRegistration(Option<Registered>),
    Home(pages::home::Message),
    Notifications(pages::notifications::Message),
    Explore(pages::public::Message),
    Local(pages::public::Message),
    Federated(pages::public::Message),
    Account(widgets::account::Message),
    Status(widgets::status::Message),
    Fetch(Vec<Url>),
    CacheStatus(Status),
    CacheNotification(Notification),
    CacheHandle(Url, Handle),
    Dialog(DialogAction),
    EditorAction(widget::text_editor::Action),
    UpdateMastodonInstance,
    None,
}

#[derive(Debug, Clone)]
pub enum DialogAction {
    Open(Dialog),
    Update(Dialog),
    Close,
    Complete,
}

#[derive(Debug, Clone)]
pub enum Dialog {
    Reply(NewStatus),
    SwitchInstance(String),
    Login(String),
    Code(String),
    Logout,
}

pub struct Flags {
    pub config: TootConfig,
    pub handler: Option<cosmic_config::Config>,
}

impl Application for AppModel {
    type Executor = cosmic::executor::multi::Executor;
    type Flags = Flags;
    type Message = Message;
    const APP_ID: &'static str = "dev.edfloreshz.Toot";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let mut nav = nav_bar::Model::default();

        let instance = instance(flags.config.server.clone());

        let mastodon = match keytar::get_password(Self::APP_ID, "mastodon-data") {
            Ok(data) => {
                if data.success {
                    let data: Data = toml::from_str(&data.password).unwrap();
                    Mastodon::from(data)
                } else {
                    Mastodon::from(Data {
                        base: instance.into(),
                        ..Default::default()
                    })
                }
            }
            Err(err) => {
                tracing::error!("{err}");
                Mastodon::from(Data {
                    base: instance.into(),
                    ..Default::default()
                })
            }
        };

        let variants = mastodon
            .data
            .token
            .is_empty()
            .then(Page::public_variants)
            .unwrap_or_else(Page::variants);

        for page in variants {
            let id = nav
                .insert()
                .text(page.to_string())
                .icon(widget::icon::from_name(page.icon()))
                .data::<Page>(page.clone())
                .id();

            if page == Page::default() {
                nav.activate(id);
            }
        }

        let about = About::default()
            .name(fl!("app-title"))
            .version("0.1.0")
            .icon(widget::icon::from_name(Self::APP_ID))
            .author("Eduardo Flores")
            .developers([("Eduardo Flores", "edfloreshz@proton.me")])
            .links([(fl!("repository"), REPOSITORY), (fl!("support"), SUPPORT)]);

        let mut app = AppModel {
            core,
            about,
            nav,
            context_page: ContextPage::default(),
            key_binds: HashMap::new(),
            dialog_pages: VecDeque::new(),
            dialog_editor: widget::text_editor::Content::default(),
            config: flags.config.clone(),
            handler: flags.handler,
            instance: flags.config.server,
            code: String::new(),
            registration: None,
            mastodon: mastodon.clone(),
            cache: Cache::new(),
            home: pages::home::Home::new(mastodon.clone()),
            notifications: pages::notifications::Notifications::new(mastodon.clone()),
            explore: pages::public::Public::new(mastodon.clone(), TimelineType::Public),
            local: pages::public::Public::new(mastodon.clone(), TimelineType::Local),
            federated: pages::public::Public::new(mastodon.clone(), TimelineType::Remote),
        };

        app.nav.activate_position(0);

        let tasks = vec![app.update_title()];

        (app, Task::batch(tasks))
    }

    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        let spacing = cosmic::theme::active().cosmic().spacing;
        let menu_bar = menu::bar(vec![menu::Tree::with_children(
            Into::<Element<Self::Message>>::into(menu::root(fl!("view"))),
            menu::items(
                &self.key_binds,
                vec![menu::Item::Button(
                    fl!("about"),
                    Some(widget::icon::from_name("help-info-symbolic").into()),
                    MenuAction::About,
                )],
            ),
        )])
        .item_height(ItemHeight::Dynamic(40))
        .item_width(ItemWidth::Uniform(260))
        .spacing(spacing.space_xxxs.into());

        vec![menu_bar.into()]
    }

    fn header_center(&self) -> Vec<Element<'_, Self::Message>> {
        vec![widget::text(self.instance.clone()).into()]
    }

    fn header_end(&self) -> Vec<Element<'_, Self::Message>> {
        if self.mastodon.data.token.is_empty() {
            vec![
                // widget::icon::from_name("network-server-symbolic")
                //     .apply(widget::button::icon)
                //     .on_press(Message::Dialog(DialogAction::Open(Dialog::SwitchInstance(
                //         self.instance.clone(),
                //     ))))
                //     .into(),
                widget::icon::from_name("system-users-symbolic")
                    .apply(widget::button::icon)
                    .on_press(Message::Dialog(DialogAction::Open(Dialog::Login(
                        self.instance.clone(),
                    ))))
                    .into(),
            ]
        } else {
            vec![widget::icon::from_name("system-log-out-symbolic")
                .apply(widget::button::icon)
                .on_press(Message::Dialog(DialogAction::Open(Dialog::Logout)))
                .into()]
        }
    }

    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav)
    }

    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<Self::Message> {
        self.nav.activate(id);
        let mut tasks = vec![];
        match self.nav.data::<Page>(id).unwrap() {
            Page::Home => tasks.push(
                self.home
                    .update(pages::home::Message::SetClient(self.mastodon.clone())),
            ),
            Page::Notifications => tasks.push(self.notifications.update(
                pages::notifications::Message::SetClient(self.mastodon.clone()),
            )),
            Page::Search => (),
            Page::Favorites => (),
            Page::Bookmarks => (),
            Page::Hashtags => (),
            Page::Lists => (),
            Page::Explore => tasks.push(
                self.explore
                    .update(pages::public::Message::SetClient(self.mastodon.clone())),
            ),
            Page::Local => tasks.push(
                self.local
                    .update(pages::public::Message::SetClient(self.mastodon.clone())),
            ),
            Page::Federated => tasks.push(
                self.federated
                    .update(pages::public::Message::SetClient(self.mastodon.clone())),
            ),
        };
        tasks.push(self.update_title());
        Task::batch(tasks)
    }

    fn context_drawer(&self) -> Option<context_drawer::ContextDrawer<'_, Self::Message>> {
        if !self.core.window.show_context {
            return None;
        }

        Some(match &self.context_page {
            ContextPage::About => context_drawer::about(
                &self.about,
                |url| Message::Open(url.to_string()),
                Message::ToggleContextDrawer,
            )
            .title(self.context_page.title()),
            ContextPage::Account(account) => {
                context_drawer::context_drawer(self.account(account), Message::ToggleContextDrawer)
                    .title(self.context_page.title())
            }
            ContextPage::Status(status) => {
                context_drawer::context_drawer(self.status(status), Message::ToggleContextDrawer)
                    .title(self.context_page.title())
            }
        })
    }

    fn dialog(&self) -> Option<Element<'_, Self::Message>> {
        let dialog_page = self.dialog_pages.front()?;

        let spacing = cosmic::theme::active().cosmic().spacing;

        let dialog = match dialog_page {
            Dialog::Reply(new_status) => widget::dialog()
                .title(fl!("reply"))
                .control(
                    widget::container(
                        widget::scrollable(
                            widget::column()
                                .push_maybe(
                                    self.cache
                                        .statuses
                                        .get(&new_status.in_reply_to_id.clone().unwrap())
                                        .map(|status| {
                                            widgets::status(
                                                status,
                                                StatusOptions::none(),
                                                &self.cache,
                                            )
                                            .map(Message::Status)
                                            .apply(widget::container)
                                            .class(cosmic::style::Container::Card)
                                        }),
                                )
                                .push(
                                    widget::text_editor(&self.dialog_editor)
                                        .height(200.)
                                        .padding(spacing.space_s)
                                        .on_action(Message::EditorAction),
                                )
                                .spacing(spacing.space_xs),
                        )
                        .width(Length::Fill),
                    )
                    .height(Length::Fixed(400.0))
                    .width(Length::Fill),
                )
                .primary_action(
                    widget::button::suggested(fl!("reply"))
                        .on_press_maybe(Some(Message::Dialog(DialogAction::Complete))),
                )
                .secondary_action(
                    widget::button::standard(fl!("cancel"))
                        .on_press(Message::Dialog(DialogAction::Close)),
                ),
            Dialog::SwitchInstance(instance) => self.switch_instance(instance.clone()),
            Dialog::Login(instance) => self.login(instance.clone()),
            Dialog::Code(code) => self.code(code.clone()),
            Dialog::Logout => self.logout(),
        };

        Some(dialog.into())
    }

    fn on_escape(&mut self) -> Task<Self::Message> {
        if self.dialog_pages.pop_front().is_some() {
            return Task::none();
        }

        if self.core.window.show_context {
            self.core.window.show_context = false;
        }

        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        match self.nav.active_data::<Page>() {
            Some(page) => match page {
                Page::Home => self.home.view(&self.cache).map(Message::Home),
                Page::Notifications => self
                    .notifications
                    .view(&self.cache)
                    .map(Message::Notifications),
                Page::Explore => self.explore.view(&self.cache).map(Message::Explore),
                Page::Local => self.local.view(&self.cache).map(Message::Local),
                Page::Federated => self.federated.view(&self.cache).map(Message::Federated),
                _ => widget::text("Not yet implemented").into(),
            },
            None => widget::text("Select a page").into(),
        }
        .apply(widget::container)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let mut subscriptions = vec![self
            .core()
            .watch_config::<TootConfig>(Self::APP_ID)
            .map(|update| Message::UpdateConfig(update.config))];

        match self.nav.active_data::<Page>() {
            Some(Page::Home) => subscriptions.push(self.home.subscription().map(Message::Home)),
            Some(Page::Notifications) => subscriptions.push(
                self.notifications
                    .subscription()
                    .map(Message::Notifications),
            ),
            Some(Page::Search) => (),
            Some(Page::Favorites) => (),
            Some(Page::Bookmarks) => (),
            Some(Page::Hashtags) => (),
            Some(Page::Lists) => (),
            Some(Page::Explore) => {
                subscriptions.push(self.explore.subscription().map(Message::Explore))
            }
            Some(Page::Local) => subscriptions.push(self.local.subscription().map(Message::Local)),
            Some(Page::Federated) => {
                subscriptions.push(self.federated.subscription().map(Message::Federated))
            }
            None => (),
        };

        if !self.mastodon.data.token.is_empty() {
            subscriptions.push(crate::subscriptions::stream_user_events(
                self.mastodon.clone(),
            ));
        }

        Subscription::batch(subscriptions)
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        let mut tasks = vec![];
        match message {
            Message::Home(message) => {
                tasks.push(self.home.update(message));
            }
            Message::Notifications(message) => {
                tasks.push(self.notifications.update(message));
            }
            Message::Explore(message) => {
                tasks.push(self.explore.update(message.clone()));
            }
            Message::Local(message) => {
                tasks.push(self.local.update(message));
            }
            Message::Federated(message) => {
                tasks.push(self.federated.update(message));
            }
            Message::Account(message) => tasks.push(widgets::account::update(message)),
            Message::Status(message) => match message {
                widgets::status::Message::Favorite(status_id, favorited) => {
                    let mastodon = self.mastodon.clone();
                    tasks.push(cosmic::task::future(async move {
                        let result = if favorited {
                            mastodon.unfavourite(&status_id).await
                        } else {
                            mastodon.favourite(&status_id).await
                        };
                        match result {
                            Ok(status) => Message::CacheStatus(status),
                            Err(err) => {
                                tracing::error!("{err}");
                                Message::None
                            }
                        }
                    }))
                }
                widgets::status::Message::Boost(status_id, boosted) => {
                    let mastodon = self.mastodon.clone();
                    tasks.push(cosmic::task::future(async move {
                        let result = if boosted {
                            mastodon.unreblog(&status_id).await
                        } else {
                            mastodon.reblog(&status_id).await
                        };
                        match result {
                            Ok(status) => Message::CacheStatus(status),
                            Err(err) => {
                                tracing::error!("{err}");
                                Message::None
                            }
                        }
                    }))
                }
                widgets::status::Message::OpenLink(_) => todo!(),
                _ => tasks.push(widgets::status::update(message)),
            },
            Message::CacheHandle(url, handle) => {
                self.cache.insert_handle(url.clone(), handle);
            }
            Message::CacheStatus(status) => {
                self.cache.insert_status(status.clone());
            }
            Message::CacheNotification(notification) => {
                self.cache.insert_notification(notification.clone());
            }
            Message::Fetch(urls) => {
                for url in urls {
                    if !self.cache.handles.contains_key(&url) {
                        tasks.push(cosmic::task::future(async move {
                            let result = match utils::get(&url).await {
                                Ok(handle) => Some((url, handle)),
                                Err(err) => {
                                    tracing::error!("Failed to fetch image: {}", err);
                                    None
                                }
                            };
                            match result {
                                Some((url, handle)) => {
                                    Message::CacheHandle(url.clone(), handle.clone())
                                }
                                None => Message::None,
                            }
                        }));
                    }
                }
            }
            Message::InstanceEdit => {
                let instance = self.instance.clone();
                if let Some(ref handler) = self.handler {
                    match self.config.set_server(handler, instance) {
                        Ok(true) => (),
                        Ok(false) => tracing::error!("Failed to write config"),
                        Err(err) => tracing::error!("{err}"),
                    }
                }
            }
            Message::RegisterMastodonClient => {
                let mut registration = Registration::new(self.config.url());
                tasks.push(cosmic::task::future(async move {
                    let scopes = Scopes::from_str("read write").unwrap();
                    match registration
                        .client_name("Toot")
                        .scopes(scopes)
                        .build()
                        .await
                    {
                        Ok(registration) => Message::StoreRegistration(Some(registration)),
                        Err(err) => {
                            tracing::error!("{err}");
                            Message::None
                        }
                    }
                }));
            }
            Message::StoreRegistration(registration) => {
                if let Some(ref registration) = registration {
                    if let Ok(url) = registration.authorize_url() {
                        if let Err(err) = open::that_detached(url) {
                            tracing::error!("{err}");
                        }
                    }
                }
                self.registration = registration;
            }
            Message::CompleteRegistration => {
                if let Some(registration) = self.registration.take() {
                    let code = self.code.clone();
                    let task = cosmic::task::future(async move {
                        match registration.complete(code).await {
                            Ok(mastodon) => Message::StoreMastodonData(mastodon),
                            Err(err) => {
                                tracing::error!("{err}");
                                Message::None
                            }
                        }
                    });
                    tasks.push(task);
                }
            }
            Message::StoreMastodonData(mastodon) => {
                let data = &toml::to_string(&mastodon.data).unwrap();
                match keytar::set_password(Self::APP_ID, "mastodon-data", data) {
                    Ok(_) => {
                        self.mastodon = mastodon;
                        self.update_navbar();
                        tasks.push(self.on_nav_select(self.nav.active()));
                    }
                    Err(err) => tracing::error!("{err}"),
                }
            }
            Message::UpdateMastodonInstance => {
                self.mastodon = Mastodon::from(Data {
                    base: self.instance().clone().into(),
                    ..Default::default()
                });
            }
            Message::Open(url) => {
                if let Err(err) = open::that_detached(url) {
                    tracing::error!("{err}")
                }
            }
            Message::ToggleContextPage(context_page) => {
                if self.context_page == context_page {
                    self.core.window.show_context = !self.core.window.show_context;
                } else {
                    self.context_page = context_page;
                    self.core.window.show_context = true;
                }
            }
            Message::ToggleContextDrawer => {
                self.core.window.show_context = !self.core.window.show_context;
            }
            Message::Dialog(action) => match action {
                DialogAction::Open(dialog) => match dialog {
                    Dialog::Reply(new_status) => {
                        if let Some(status) = new_status.status.clone() {
                            self.dialog_editor = widget::text_editor::Content::with_text(&status);
                        }
                        self.dialog_pages.push_back(Dialog::Reply(new_status))
                    }
                    _ => self.dialog_pages.push_back(dialog),
                },
                DialogAction::Update(dialog_page) => {
                    self.dialog_pages[0] = dialog_page;
                }
                DialogAction::Close => {
                    self.dialog_pages.pop_front();
                }
                DialogAction::Complete => {
                    if let Some(dialog_page) = self.dialog_pages.pop_front() {
                        match dialog_page {
                            Dialog::Reply(mut new_status) => {
                                new_status.status = Some(self.dialog_editor.text());
                                let mastodon = self.mastodon.clone();
                                tasks.push(cosmic::task::future(async move {
                                    match mastodon.new_status(new_status).await {
                                        Ok(status) => Message::CacheStatus(status),
                                        Err(err) => {
                                            tracing::error!("{err}");
                                            Message::None
                                        }
                                    }
                                }));
                            }
                            Dialog::SwitchInstance(instance) => {
                                self.instance = instance;
                                tasks.push(self.update(Message::InstanceEdit));
                                tasks.push(self.update(Message::UpdateMastodonInstance))
                            }
                            Dialog::Login(instance) => {
                                self.instance = instance;
                                tasks.push(self.update(Message::InstanceEdit));
                                tasks.push(self.update(Message::RegisterMastodonClient));
                                tasks.push(self.update(Message::Dialog(DialogAction::Open(
                                    Dialog::Code(String::new()),
                                ))))
                            }
                            Dialog::Code(code) => {
                                self.code = code;
                                tasks.push(self.update(Message::CompleteRegistration))
                            }
                            Dialog::Logout => {
                                self.mastodon = Mastodon::from(Data {
                                    base: self.instance().into(),
                                    ..Default::default()
                                });
                                self.update_navbar();
                                if let Err(err) =
                                    keytar::delete_password(Self::APP_ID, "mastodon-data")
                                {
                                    tracing::error!("{err}");
                                }
                            }
                        }
                    }
                }
            },
            Message::EditorAction(action) => {
                self.dialog_editor.perform(action);
            }
            Message::UpdateConfig(config) => {
                self.config = config;
            }
            Message::None => (),
        }
        Task::batch(tasks)
    }
}

impl AppModel {
    pub fn update_title(&mut self) -> Task<Message> {
        let mut window_title = fl!("app-title");
        if let Some(page) = self.nav.text(self.nav.active()) {
            window_title.push_str(" — ");
            window_title.push_str(page);
        }
        if let Some(id) = self.core.main_window_id() {
            self.set_window_title(window_title, id)
        } else {
            Task::none()
        }
    }

    fn switch_instance(&self, instance: String) -> widget::Dialog<'_, Message> {
        widget::dialog()
            .title(fl!("server-question"))
            .body(fl!("server-description"))
            .icon(widget::icon::from_name("network-server-symbolic"))
            .control(
                widget::text_input(fl!("server-url"), instance)
                    .on_input(|instance| {
                        Message::Dialog(DialogAction::Update(Dialog::SwitchInstance(instance)))
                    })
                    .on_submit(|_| Message::Dialog(DialogAction::Complete)),
            )
            .primary_action(
                widget::button::suggested(fl!("confirm"))
                    .on_press(Message::Dialog(DialogAction::Complete)),
            )
            .secondary_action(
                widget::button::standard(fl!("cancel"))
                    .on_press(Message::Dialog(DialogAction::Close)),
            )
    }

    fn login(&self, instance: String) -> widget::Dialog<'_, Message> {
        widget::dialog()
            .title(fl!("server-question"))
            .body(fl!("server-description"))
            .icon(widget::icon::from_name("network-server-symbolic"))
            .control(
                widget::text_input(fl!("server-url"), instance.clone())
                    .on_input(move |instance| {
                        Message::Dialog(DialogAction::Update(Dialog::Login(instance.clone())))
                    })
                    .on_submit(|_| Message::Dialog(DialogAction::Complete)),
            )
            .primary_action(
                widget::button::suggested(fl!("continue"))
                    .on_press(Message::Dialog(DialogAction::Complete)),
            )
            .secondary_action(
                widget::button::standard(fl!("cancel"))
                    .on_press(Message::Dialog(DialogAction::Close)),
            )
    }

    fn code(&self, code: String) -> widget::Dialog<'_, Message> {
        widget::dialog()
            .title(fl!("confirm-authorization"))
            .body(fl!("confirm-authorization-description"))
            .icon(widget::icon::from_name("network-server-symbolic"))
            .control(
                widget::text_input(fl!("authorization-code"), code.clone())
                    .on_input(|code| Message::Dialog(DialogAction::Update(Dialog::Code(code))))
                    .on_submit(|_| Message::Dialog(DialogAction::Complete)),
            )
            .primary_action(
                widget::button::suggested(fl!("confirm"))
                    .on_press(Message::Dialog(DialogAction::Complete)),
            )
            .secondary_action(
                widget::button::standard(fl!("cancel"))
                    .on_press(Message::Dialog(DialogAction::Close)),
            )
    }

    fn logout(&self) -> widget::Dialog<'_, Message> {
        widget::dialog()
            .title(fl!("logout-question"))
            .body(fl!("logout-description"))
            .icon(widget::icon::from_name("system-log-out-symbolic"))
            .primary_action(
                widget::button::suggested(fl!("continue"))
                    .on_press(Message::Dialog(DialogAction::Complete)),
            )
            .secondary_action(
                widget::button::standard(fl!("cancel"))
                    .on_press(Message::Dialog(DialogAction::Close)),
            )
    }

    fn status(&self, id: &StatusId) -> Element<'_, Message> {
        let status = self.cache.statuses.get(&id.to_string()).map(|status| {
            crate::widgets::status(
                status,
                StatusOptions::new(true, true, true, false),
                &self.cache,
            )
            .map(pages::home::Message::Status)
            .map(Message::Home)
            .apply(widget::container)
            .class(cosmic::theme::Container::Dialog)
        });
        widget::column().push_maybe(status).into()
    }

    fn account<'a>(&'a self, account: &'a Account) -> Element<'a, Message> {
        crate::widgets::account(account, &self.cache.handles).map(Message::Account)
    }
}

fn instance(instance: impl Into<String>) -> String {
    let instance: String = instance.into();
    instance
        .is_empty()
        .then(|| format!("https://{}", "mastodon.social"))
        .unwrap_or(format!("https://{}", instance))
}

impl AppModel
where
    Self: Application,
{
    fn instance(&self) -> String {
        instance(self.instance.clone())
    }

    fn update_navbar(&mut self) {
        self.nav.clear();

        let variants = self
            .mastodon
            .data
            .token
            .is_empty()
            .then(Page::public_variants)
            .unwrap_or_else(Page::variants);

        for page in variants {
            self.nav
                .insert()
                .text(page.to_string())
                .icon(widget::icon::from_name(page.icon()))
                .data::<Page>(page.clone());

            self.nav.activate_position(0);
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum ContextPage {
    #[default]
    About,
    Account(Account),
    Status(StatusId),
}

impl ContextPage {
    fn title(&self) -> String {
        match self {
            ContextPage::About => fl!("about"),
            ContextPage::Account(_) => fl!("profile"),
            ContextPage::Status(_) => fl!("status"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    About,
}

impl menu::action::MenuAction for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            MenuAction::About => Message::ToggleContextPage(ContextPage::About),
        }
    }
}
