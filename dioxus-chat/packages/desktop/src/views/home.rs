use dioxus::prelude::*;
use ui::ChatContainer;

#[component]
pub fn Home() -> Element {
    rsx! {
        ChatContainer {}
    }
}
