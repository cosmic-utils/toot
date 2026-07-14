// SPDX-License-Identifier: {{LICENSE}}

use crate::cache::Cache;
use crate::client::{Client, Session, Sessions};
use crate::config::TootConfig;
use crate::features::compose;
use crate::features::status::StatusOptions;
use crate::features::timeline::{Timeline, TimelineKind};
use crate::features::{accounts, hashtags, lists, notifications, search, settings, status, timeline};
use crate::fl;
use cosmic::app::{context_drawer, Core, Task};
use cosmic::cosmic_config;
use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::{Length, Subscription};
use cosmic::widget::about::About;
use cosmic::widget::image::Handle;
use cosmic::widget::menu::{ItemHeight, ItemWidth};
use cosmic::widget::toaster::{Toast, ToastId, Toasts};
use cosmic::widget::{self, menu, nav_bar};
use cosmic::{Application, ApplicationExt, Apply, Element};
use megalodon::entities::{Account, Notification, Status};
use megalodon::megalodon::PostStatusInputOptions;
use megalodon::oauth::AppData;

use std::collections::{HashMap, VecDeque};
use std::fmt::Display;

const REPOSITORY: &str = "https://github.com/edfloreshz/toot";
const SUPPORT: &str = "https://github.com/edfloreshz/toot/issues";

/// A nav-bar destination. Some variants are not yet backed by a feature — see
/// `view`/`on_nav_select`/`subscription`'s fallback arms.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum Page {
    #[default]
    Home,
    Notifications,
    Search,
    Favorites,
    Bookmarks,
    Hashtags,
    Lists,
    Explore,
    Local,
    Federated,
}

impl Display for Page {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Page::Home => write!(f, "{}", fl!("home")),
            Page::Notifications => write!(f, "{}", fl!("notifications")),
            Page::Search => write!(f, "{}", fl!("search")),
            Page::Favorites => write!(f, "{}", fl!("favorites")),
            Page::Bookmarks => write!(f, "{}", fl!("bookmarks")),
            Page::Hashtags => write!(f, "{}", fl!("hashtags")),
            Page::Lists => write!(f, "{}", fl!("lists")),
            Page::Explore => write!(f, "{}", fl!("explore")),
            Page::Local => write!(f, "{}", fl!("local")),
            Page::Federated => write!(f, "{}", fl!("federated")),
        }
    }
}

impl Page {
    pub fn public_variants() -> Vec<Page> {
        vec![
            Self::Explore,
            Self::Local,
            Self::Federated,
            Self::Search,
            Self::Hashtags,
        ]
    }

    pub fn variants() -> Vec<Page> {
        vec![
            Self::Home,
            Self::Notifications,
            Self::Search,
            Self::Favorites,
            Self::Bookmarks,
            Self::Hashtags,
            Self::Lists,
            Self::Explore,
            Self::Local,
            Self::Federated,
        ]
    }

    pub fn icon(&self) -> &str {
        match self {
            Page::Home => "user-home-symbolic",
            Page::Notifications => "emblem-important-symbolic",
            Page::Search => "folder-saved-search-symbolic",
            Page::Favorites => "starred-symbolic",
            Page::Bookmarks => "bookmark-new-symbolic",
            Page::Hashtags => "lang-include-symbolic",
            Page::Lists => "view-list-symbolic",
            Page::Explore => "find-location-symbolic",
            Page::Local => "network-server-symbolic",
            Page::Federated => "network-workgroup-symbolic",
        }
    }
}

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
    registration: Option<AppData>,
    mastodon: Client,
    /// All saved accounts (including the currently active one) and which
    /// index is active, persisted to the keychain as a whole.
    sessions: Sessions,
    cache: Cache,
    toasts: Toasts<Message>,
    /// The instance's reported maximum status length, used for the compose
    /// dialog's character counter. Defaults to Mastodon's standard limit
    /// until the real value is fetched after login.
    max_characters: u32,
    home: Timeline,
    notifications: notifications::Notifications,
    explore: Timeline,
    local: Timeline,
    federated: Timeline,
    favorites: Timeline,
    bookmarks: Timeline,
    hashtags: hashtags::Hashtags,
    lists: lists::Lists,
    search: search::Search,
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
    StoreMastodonData(Client),
    StoreRegistration(Option<AppData>),
    Home(timeline::Message),
    Notifications(notifications::Message),
    Explore(timeline::Message),
    Local(timeline::Message),
    Federated(timeline::Message),
    Favorites(timeline::Message),
    Bookmarks(timeline::Message),
    Hashtags(hashtags::Message),
    Lists(lists::Message),
    Search(search::Message),
    Settings(settings::Message),
    Account(accounts::Message),
    Status(status::Message),
    Fetch(Vec<String>),
    CacheStatus(Status),
    CacheNotification(Notification),
    CacheRelationship(megalodon::entities::Relationship),
    CacheHandle(String, Handle),
    Dialog(DialogAction),
    EditorAction(widget::text_editor::Action),
    UpdateMastodonInstance,
    /// The authenticated account, cached right after login (and at startup
    /// if a saved session is restored) so posts/relationships can be
    /// compared against "me".
    SetAccount(Account),
    /// The instance's max status length, fetched right after login.
    SetMaxCharacters(u32),
    /// A status was deleted on the server; remove it from the cache.
    StatusDeleted(String),
    /// A recoverable error to surface to the user as a toast instead of
    /// only logging it and silently dropping the failed action.
    Error(String),
    CloseToast(ToastId),
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
    Compose(compose::State),
    SwitchInstance(String),
    Login(String),
    Code(String),
    Logout,
    DeleteStatus(String),
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

        let sessions = match keytar::get_password(Self::APP_ID, "mastodon-data") {
            Ok(data) if data.success => Sessions::parse(&data.password).unwrap_or_default(),
            Ok(_) => Sessions::default(),
            Err(err) => {
                tracing::error!("{err}");
                Sessions::default()
            }
        };

        let mastodon = match sessions.active_session() {
            Some(session) => Client::new(session.base_url.clone(), Some(session.token.clone())),
            None => Client::new(instance.clone(), None),
        };

        let variants = (!mastodon.is_authenticated())
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
            sessions,
            cache: {
                let mut cache = Cache::new();
                cache.hide_boosts = flags.config.hide_boosts;
                cache.hide_replies = flags.config.hide_replies;
                cache
            },
            toasts: Toasts::new(Message::CloseToast),
            max_characters: 500,
            home: Timeline::new(mastodon.clone(), TimelineKind::Home),
            notifications: notifications::Notifications::new(mastodon.clone()),
            explore: Timeline::new(mastodon.clone(), TimelineKind::Public),
            local: Timeline::new(mastodon.clone(), TimelineKind::Local),
            federated: Timeline::new(mastodon.clone(), TimelineKind::Federated),
            favorites: Timeline::new(mastodon.clone(), TimelineKind::Favorites),
            bookmarks: Timeline::new(mastodon.clone(), TimelineKind::Bookmarks),
            hashtags: hashtags::Hashtags::new(mastodon.clone()),
            lists: lists::Lists::new(mastodon.clone()),
            search: search::Search::new(mastodon.clone()),
        };

        app.nav.activate_position(0);

        let mut tasks = vec![app.update_title()];
        if mastodon.is_authenticated() {
            tasks.push(fetch_session_info(mastodon));
        }

        (app, Task::batch(tasks))
    }

    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        let spacing = cosmic::theme::active().cosmic().spacing;
        let menu_bar = menu::bar(vec![menu::Tree::with_children(
            Into::<Element<Self::Message>>::into(menu::root(fl!("view"))),
            menu::items(
                &self.key_binds,
                vec![
                    menu::Item::Button(
                        "Settings".to_string(),
                        Some(widget::icon::from_name("preferences-system-symbolic").into()),
                        MenuAction::Settings,
                    ),
                    menu::Item::Button(
                        fl!("about"),
                        Some(widget::icon::from_name("help-info-symbolic").into()),
                        MenuAction::About,
                    ),
                ],
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
        if !self.mastodon.is_authenticated() {
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
            vec![
                widget::icon::from_name("list-add-symbolic")
                    .apply(widget::button::icon)
                    .on_press(Message::Dialog(DialogAction::Open(Dialog::Compose(
                        compose::State::default(),
                    ))))
                    .into(),
                widget::icon::from_name("system-log-out-symbolic")
                    .apply(widget::button::icon)
                    .on_press(Message::Dialog(DialogAction::Open(Dialog::Logout)))
                    .into(),
            ]
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
                    .update(timeline::Message::SetClient(self.mastodon.clone())),
            ),
            Page::Notifications => tasks.push(self.notifications.update(
                notifications::Message::SetClient(self.mastodon.clone()),
            )),
            Page::Search => tasks.push(
                self.search
                    .update(search::Message::SetClient(self.mastodon.clone())),
            ),
            Page::Favorites => tasks.push(
                self.favorites
                    .update(timeline::Message::SetClient(self.mastodon.clone())),
            ),
            Page::Bookmarks => tasks.push(
                self.bookmarks
                    .update(timeline::Message::SetClient(self.mastodon.clone())),
            ),
            Page::Hashtags => tasks.push(
                self.hashtags
                    .update(hashtags::Message::SetClient(self.mastodon.clone())),
            ),
            Page::Lists => tasks.push(
                self.lists
                    .update(lists::Message::SetClient(self.mastodon.clone())),
            ),
            Page::Explore => tasks.push(
                self.explore
                    .update(timeline::Message::SetClient(self.mastodon.clone())),
            ),
            Page::Local => tasks.push(
                self.local
                    .update(timeline::Message::SetClient(self.mastodon.clone())),
            ),
            Page::Federated => tasks.push(
                self.federated
                    .update(timeline::Message::SetClient(self.mastodon.clone())),
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
            ContextPage::Settings => {
                let content = settings::view(&self.config, &self.sessions.sessions, self.sessions.active)
                    .map(Message::Settings);
                context_drawer::context_drawer(content, Message::ToggleContextDrawer)
                    .title(self.context_page.title())
            }
        })
    }

    fn dialog(&self) -> Option<Element<'_, Self::Message>> {
        let dialog_page = self.dialog_pages.front()?;

        let dialog = match dialog_page {
            Dialog::Compose(state) => {
                let reply_preview = state.in_reply_to_id.as_ref().and_then(|id| {
                    self.cache.statuses.get(id).map(|reply_target| {
                        status::status(reply_target, StatusOptions::none(), &self.cache)
                            .map(Message::Status)
                            .apply(widget::container)
                            .class(cosmic::style::Container::Card)
                            .into()
                    })
                });
                compose::view(state, reply_preview, &self.dialog_editor, self.max_characters)
            }
            Dialog::SwitchInstance(instance) => self.switch_instance(instance.clone()),
            Dialog::Login(instance) => self.login(instance.clone()),
            Dialog::Code(code) => self.code(code.clone()),
            Dialog::Logout => self.logout(),
            Dialog::DeleteStatus(id) => self.delete_status(id.clone()),
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
        let content = match self.nav.active_data::<Page>() {
            Some(page) => match page {
                Page::Home => self.home.view(&self.cache).map(Message::Home),
                Page::Notifications => self
                    .notifications
                    .view(&self.cache)
                    .map(Message::Notifications),
                Page::Explore => self.explore.view(&self.cache).map(Message::Explore),
                Page::Local => self.local.view(&self.cache).map(Message::Local),
                Page::Federated => self.federated.view(&self.cache).map(Message::Federated),
                Page::Favorites => self.favorites.view(&self.cache).map(Message::Favorites),
                Page::Bookmarks => self.bookmarks.view(&self.cache).map(Message::Bookmarks),
                Page::Hashtags => self.hashtags.view(&self.cache).map(Message::Hashtags),
                Page::Lists => self.lists.view(&self.cache).map(Message::Lists),
                Page::Search => self.search.view(&self.cache).map(Message::Search),
            },
            None => widget::text("Select a page").into(),
        }
        .apply(widget::container)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);

        widget::toaster(&self.toasts, content)
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
            Some(Page::Search) => {
                subscriptions.push(self.search.subscription().map(Message::Search))
            }
            Some(Page::Favorites) => {
                subscriptions.push(self.favorites.subscription().map(Message::Favorites))
            }
            Some(Page::Bookmarks) => {
                subscriptions.push(self.bookmarks.subscription().map(Message::Bookmarks))
            }
            Some(Page::Hashtags) => {
                subscriptions.push(self.hashtags.subscription().map(Message::Hashtags))
            }
            Some(Page::Lists) => subscriptions.push(self.lists.subscription().map(Message::Lists)),
            Some(Page::Explore) => {
                subscriptions.push(self.explore.subscription().map(Message::Explore))
            }
            Some(Page::Local) => subscriptions.push(self.local.subscription().map(Message::Local)),
            Some(Page::Federated) => {
                subscriptions.push(self.federated.subscription().map(Message::Federated))
            }
            None => (),
        };

        if self.mastodon.is_authenticated() {
            subscriptions.push(crate::streaming::stream_user_events(self.mastodon.clone()));
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
            Message::Favorites(message) => {
                tasks.push(self.favorites.update(message));
            }
            Message::Bookmarks(message) => {
                tasks.push(self.bookmarks.update(message));
            }
            Message::Hashtags(message) => {
                tasks.push(self.hashtags.update(message));
            }
            Message::Lists(message) => {
                tasks.push(self.lists.update(message));
            }
            Message::Search(message) => {
                tasks.push(self.search.update(message));
            }
            Message::Settings(message) => match message {
                settings::Message::ToggleHideBoosts(hide) => {
                    self.cache.hide_boosts = hide;
                    if let Some(ref handler) = self.handler {
                        if let Err(err) = self.config.set_hide_boosts(handler, hide) {
                            tracing::error!("{err}");
                        }
                    }
                }
                settings::Message::ToggleHideReplies(hide) => {
                    self.cache.hide_replies = hide;
                    if let Some(ref handler) = self.handler {
                        if let Err(err) = self.config.set_hide_replies(handler, hide) {
                            tracing::error!("{err}");
                        }
                    }
                }
                settings::Message::SwitchAccount(index) => tasks.push(self.switch_account(index)),
                settings::Message::RemoveAccount(index) => tasks.push(self.remove_account(index)),
                settings::Message::AddAccount => {
                    tasks.push(self.update(Message::Dialog(DialogAction::Open(Dialog::Login(
                        self.instance.clone(),
                    )))));
                }
            },
            Message::Account(message) => match message {
                accounts::Message::Follow(id, following) => {
                    let mastodon = self.mastodon.clone();
                    tasks.push(cosmic::task::future(async move {
                        let result = if following {
                            mastodon.unfollow_account(id).await
                        } else {
                            mastodon.follow_account(id, None).await
                        };
                        match result {
                            Ok(response) => Message::CacheRelationship(response.json),
                            Err(err) => Message::Error(format!("Couldn't update follow: {err}")),
                        }
                    }))
                }
                accounts::Message::Mute(id, muting) => {
                    let mastodon = self.mastodon.clone();
                    tasks.push(cosmic::task::future(async move {
                        let result = if muting {
                            mastodon.unmute_account(id).await
                        } else {
                            mastodon.mute_account(id, true).await
                        };
                        match result {
                            Ok(response) => Message::CacheRelationship(response.json),
                            Err(err) => Message::Error(format!("Couldn't update mute: {err}")),
                        }
                    }))
                }
                accounts::Message::Block(id, blocking) => {
                    let mastodon = self.mastodon.clone();
                    tasks.push(cosmic::task::future(async move {
                        let result = if blocking {
                            mastodon.unblock_account(id).await
                        } else {
                            mastodon.block_account(id).await
                        };
                        match result {
                            Ok(response) => Message::CacheRelationship(response.json),
                            Err(err) => Message::Error(format!("Couldn't update block: {err}")),
                        }
                    }))
                }
                _ => tasks.push(accounts::update(message)),
            },
            Message::Status(message) => match message {
                status::Message::Favorite(status_id, favorited) => {
                    let mastodon = self.mastodon.clone();
                    tasks.push(cosmic::task::future(async move {
                        let result = if favorited {
                            mastodon.unfavourite_status(status_id).await
                        } else {
                            mastodon.favourite_status(status_id).await
                        };
                        match result {
                            Ok(response) => Message::CacheStatus(response.json),
                            Err(err) => Message::Error(format!("Couldn't update favorite: {err}")),
                        }
                    }))
                }
                status::Message::Boost(status_id, boosted) => {
                    let mastodon = self.mastodon.clone();
                    tasks.push(cosmic::task::future(async move {
                        let result = if boosted {
                            mastodon.unreblog_status(status_id).await
                        } else {
                            mastodon.reblog_status(status_id).await
                        };
                        match result {
                            Ok(response) => Message::CacheStatus(response.json),
                            Err(err) => Message::Error(format!("Couldn't update boost: {err}")),
                        }
                    }))
                }
                status::Message::Bookmark(status_id, bookmarked) => {
                    let mastodon = self.mastodon.clone();
                    tasks.push(cosmic::task::future(async move {
                        let result = if bookmarked {
                            mastodon.unbookmark_status(status_id).await
                        } else {
                            mastodon.bookmark_status(status_id).await
                        };
                        match result {
                            Ok(response) => Message::CacheStatus(response.json),
                            Err(err) => Message::Error(format!("Couldn't update bookmark: {err}")),
                        }
                    }))
                }
                _ => tasks.push(status::update(message)),
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
            Message::CacheRelationship(relationship) => {
                self.cache.insert_relationship(relationship);
            }
            Message::Fetch(urls) => {
                for url in urls {
                    if !self.cache.handles.contains_key(&url) {
                        tasks.push(cosmic::task::future(async move {
                            let result = match crate::cache::get(&url).await {
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
                    if let Err(err) = self.config.set_server(handler, instance) {
                        tracing::error!("{err}")
                    }
                }
            }
            Message::RegisterMastodonClient => {
                let instance = self.instance();
                tasks.push(cosmic::task::future(async move {
                    let client = Client::new(instance, None);
                    let options = megalodon::megalodon::AppInputOptions {
                        scopes: Some(
                            ["read", "write", "follow"]
                                .into_iter()
                                .map(String::from)
                                .collect(),
                        ),
                        ..Default::default()
                    };
                    match client.register_app("Toot".to_string(), &options).await {
                        Ok(app_data) => Message::StoreRegistration(Some(app_data)),
                        Err(err) => Message::Error(format!("Couldn't register with server: {err}")),
                    }
                }));
            }
            Message::StoreRegistration(registration) => {
                if let Some(ref registration) = registration {
                    if let Some(url) = registration.url.clone() {
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
                    let instance = self.instance();
                    let task = cosmic::task::future(async move {
                        let client = Client::new(instance.clone(), None);
                        match client
                            .fetch_access_token(
                                registration.client_id,
                                registration.client_secret,
                                code,
                                megalodon::default::NO_REDIRECT.to_string(),
                            )
                            .await
                        {
                            Ok(token) => Message::StoreMastodonData(Client::new(
                                instance,
                                Some(token.access_token),
                            )),
                            Err(err) => Message::Error(format!("Couldn't complete login: {err}")),
                        }
                    });
                    tasks.push(task);
                }
            }
            Message::StoreMastodonData(mastodon) => {
                let session = Session {
                    base_url: mastodon.base_url.clone(),
                    token: mastodon.token.clone().unwrap_or_default(),
                };
                self.sessions.upsert_active(session);
                match self.persist_sessions() {
                    Ok(_) => {
                        self.mastodon = mastodon;
                        self.update_navbar();
                        tasks.push(self.on_nav_select(self.nav.active()));
                        tasks.push(self.update_all_clients());
                        tasks.push(fetch_session_info(self.mastodon.clone()));
                    }
                    Err(err) => tasks.push(cosmic::task::message(Message::Error(format!(
                        "Couldn't save session: {err}"
                    )))),
                }
            }
            Message::UpdateMastodonInstance => {
                self.mastodon = Client::new(self.instance(), None);
            }
            Message::SetAccount(account) => {
                self.cache.me = Some(account);
            }
            Message::SetMaxCharacters(max_characters) => {
                self.max_characters = max_characters;
            }
            Message::StatusDeleted(id) => {
                self.cache.statuses.remove(&id);
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
                    if let ContextPage::Account(account) = &context_page {
                        if self.mastodon.is_authenticated() {
                            let mastodon = self.mastodon.clone();
                            let id = account.id.clone();
                            tasks.push(cosmic::task::future(async move {
                                match mastodon.get_relationships(vec![id]).await {
                                    Ok(response) => response
                                        .json
                                        .into_iter()
                                        .next()
                                        .map(Message::CacheRelationship)
                                        .unwrap_or(Message::None),
                                    Err(err) => Message::Error(format!(
                                        "Couldn't load relationship: {err}"
                                    )),
                                }
                            }));
                        }
                    }
                    self.context_page = context_page;
                    self.core.window.show_context = true;
                }
            }
            Message::ToggleContextDrawer => {
                self.core.window.show_context = !self.core.window.show_context;
            }
            Message::Dialog(action) => match action {
                DialogAction::Open(dialog) => match dialog {
                    Dialog::Compose(state) => {
                        self.dialog_editor = widget::text_editor::Content::with_text(
                            state.text.as_deref().unwrap_or(""),
                        );
                        self.dialog_pages.push_back(Dialog::Compose(state))
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
                            Dialog::Compose(state) => {
                                let text = self.dialog_editor.text();
                                let mastodon = self.mastodon.clone();
                                tasks.push(cosmic::task::future(async move {
                                    let options = PostStatusInputOptions {
                                        in_reply_to_id: state.in_reply_to_id,
                                        spoiler_text: state
                                            .content_warning
                                            .then_some(state.spoiler_text),
                                        visibility: Some(state.visibility),
                                        ..Default::default()
                                    };
                                    match mastodon.post_status(text, Some(&options)).await {
                                        Ok(response) => match response.json {
                                            megalodon::megalodon::PostStatusOutput::Status(
                                                status,
                                            ) => Message::CacheStatus(status),
                                            megalodon::megalodon::PostStatusOutput::ScheduledStatus(
                                                _,
                                            ) => Message::None,
                                        },
                                        Err(err) => Message::Error(format!("Couldn't post: {err}")),
                                    }
                                }));
                            }
                            Dialog::DeleteStatus(id) => {
                                let mastodon = self.mastodon.clone();
                                tasks.push(cosmic::task::future(async move {
                                    match mastodon.delete_status(id.clone()).await {
                                        Ok(_) => Message::StatusDeleted(id),
                                        Err(err) => {
                                            Message::Error(format!("Couldn't delete post: {err}"))
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
                                tasks.push(self.remove_account(self.sessions.active));
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
            Message::Error(message) => {
                tracing::error!("{message}");
                tasks.push(self.toasts.push(Toast::new(message)).map(cosmic::Action::App));
            }
            Message::CloseToast(id) => {
                self.toasts.remove(id);
            }
            Message::None => (),
        }
        Task::batch(tasks)
    }
}

impl AppModel {
    /// Persist the full account list (and which one is active) to the keychain.
    fn persist_sessions(&self) -> Result<(), String> {
        let data = serde_json::to_string(&self.sessions).map_err(|err| err.to_string())?;
        keytar::set_password(Self::APP_ID, "mastodon-data", &data).map_err(|err| err.to_string())
    }

    /// Push the active client to every feature that holds its own copy, so
    /// switching accounts doesn't leave a page talking to the old session.
    fn update_all_clients(&mut self) -> Task<Message> {
        let mastodon = self.mastodon.clone();
        Task::batch(vec![
            self.home.update(timeline::Message::SetClient(mastodon.clone())),
            self.notifications
                .update(notifications::Message::SetClient(mastodon.clone())),
            self.explore.update(timeline::Message::SetClient(mastodon.clone())),
            self.local.update(timeline::Message::SetClient(mastodon.clone())),
            self.federated.update(timeline::Message::SetClient(mastodon.clone())),
            self.favorites.update(timeline::Message::SetClient(mastodon.clone())),
            self.bookmarks.update(timeline::Message::SetClient(mastodon.clone())),
            self.hashtags.update(hashtags::Message::SetClient(mastodon.clone())),
            self.lists.update(lists::Message::SetClient(mastodon.clone())),
            self.search.update(search::Message::SetClient(mastodon)),
        ])
    }

    /// Switch to the saved account at `index`, resetting per-account state.
    fn switch_account(&mut self, index: usize) -> Task<Message> {
        let Some(session) = self.sessions.sessions.get(index) else {
            return Task::none();
        };
        self.sessions.active = index;
        self.mastodon = Client::new(session.base_url.clone(), Some(session.token.clone()));
        self.cache.clear();
        self.cache.hide_boosts = self.config.hide_boosts;
        self.cache.hide_replies = self.config.hide_replies;
        self.update_navbar();

        let mut tasks = vec![self.update_all_clients(), self.on_nav_select(self.nav.active())];
        if let Err(err) = self.persist_sessions() {
            tasks.push(cosmic::task::message(Message::Error(format!(
                "Couldn't save session: {err}"
            ))));
        }
        tasks.push(fetch_session_info(self.mastodon.clone()));
        Task::batch(tasks)
    }

    /// Remove the saved account at `index`. If it was active, switches to
    /// another remaining account or falls back to a logged-out client.
    fn remove_account(&mut self, index: usize) -> Task<Message> {
        let was_active = index == self.sessions.active;
        self.sessions.remove(index);

        let mut tasks = vec![];
        if let Err(err) = self.persist_sessions() {
            tasks.push(cosmic::task::message(Message::Error(format!(
                "Couldn't save session: {err}"
            ))));
        }

        if was_active {
            match self.sessions.active_session().cloned() {
                Some(session) => {
                    self.mastodon = Client::new(session.base_url, Some(session.token));
                }
                None => {
                    self.mastodon = Client::new(self.instance(), None);
                }
            }
            self.cache.clear();
            self.cache.hide_boosts = self.config.hide_boosts;
            self.cache.hide_replies = self.config.hide_replies;
            self.update_navbar();
            tasks.push(self.update_all_clients());
            tasks.push(self.on_nav_select(self.nav.active()));
        }
        Task::batch(tasks)
    }

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

    fn delete_status(&self, _id: String) -> widget::Dialog<'_, Message> {
        widget::dialog()
            .title(fl!("delete-status-question"))
            .body(fl!("delete-status-description"))
            .icon(widget::icon::from_name("user-trash-symbolic"))
            .primary_action(
                widget::button::destructive(fl!("delete"))
                    .on_press(Message::Dialog(DialogAction::Complete)),
            )
            .secondary_action(
                widget::button::standard(fl!("cancel"))
                    .on_press(Message::Dialog(DialogAction::Close)),
            )
    }

    fn status(&self, id: &String) -> Element<'_, Message> {
        let status = self.cache.statuses.get(id).map(|status| {
            status::status(status, StatusOptions::new(true, true, true, false), &self.cache)
                .map(timeline::Message::Status)
                .map(Message::Home)
                .apply(widget::container)
                .class(cosmic::theme::Container::Dialog(false))
        });
        widget::column![status].into()
    }

    fn account<'a>(&'a self, account: &'a Account) -> Element<'a, Message> {
        accounts::account(account, &self.cache).map(Message::Account)
    }
}

/// Fetch the authenticated account and the instance's status length limit,
/// used to gate the compose dialog's delete action and character counter.
fn fetch_session_info(mastodon: Client) -> Task<Message> {
    let account_client = mastodon.clone();
    let instance_client = mastodon;
    Task::batch(vec![
        cosmic::task::future(async move {
            match account_client.verify_account_credentials().await {
                Ok(response) => Message::SetAccount(response.json),
                Err(err) => Message::Error(format!("Couldn't load account: {err}")),
            }
        }),
        cosmic::task::future(async move {
            match instance_client.get_instance().await {
                Ok(response) => {
                    Message::SetMaxCharacters(response.json.configuration.statuses.max_characters)
                }
                Err(err) => Message::Error(format!("Couldn't load instance info: {err}")),
            }
        }),
    ])
}

fn instance(instance: impl Into<String>) -> String {
    let instance: String = instance.into();
    let instance = instance
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    if instance.is_empty() {
        format!("https://{}", "mastodon.social")
    } else {
        format!("https://{}", instance)
    }
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

        let variants = (!self.mastodon.is_authenticated())
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
    Status(String),
    Settings,
}

impl ContextPage {
    fn title(&self) -> String {
        match self {
            ContextPage::About => fl!("about"),
            ContextPage::Account(_) => fl!("profile"),
            ContextPage::Status(_) => fl!("status"),
            ContextPage::Settings => "Settings".to_string(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    About,
    Settings,
}

impl menu::action::MenuAction for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            MenuAction::About => Message::ToggleContextPage(ContextPage::About),
            MenuAction::Settings => Message::ToggleContextPage(ContextPage::Settings),
        }
    }
}
