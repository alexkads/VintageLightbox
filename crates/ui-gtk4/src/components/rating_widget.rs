//! Rating Widget Component
//!
//! Star-based rating selector (0-5 stars).

use gtk4::prelude::*;
use relm4::prelude::*;

/// Rating widget component
pub struct RatingWidget {
    rating: i32,
    hover_rating: Option<i32>,
}

/// Messages for the rating widget
#[derive(Debug)]
pub enum RatingMsg {
    SetRating(i32),
    HoverRating(i32),
    HoverLeave,
    Click(i32),
}

/// Output messages from the rating widget
#[derive(Debug)]
pub enum RatingOutput {
    RatingChanged(i32),
}

#[relm4::component(pub)]
impl SimpleComponent for RatingWidget {
    type Init = i32;
    type Input = RatingMsg;
    type Output = RatingOutput;

    view! {
        gtk4::Box {
            set_orientation: gtk4::Orientation::Horizontal,
            set_spacing: 2,
            add_css_class: "rating-widget",
            
            // 5 star buttons
            gtk4::Button {
                set_label: if model.effective_rating() >= 1 { "★" } else { "☆" },
                add_css_class: "flat",
                add_css_class: if model.effective_rating() >= 1 { "star-filled" } else { "star-empty" },
                connect_clicked => RatingMsg::Click(1),
            },
            
            gtk4::Button {
                set_label: if model.effective_rating() >= 2 { "★" } else { "☆" },
                add_css_class: "flat",
                add_css_class: if model.effective_rating() >= 2 { "star-filled" } else { "star-empty" },
                connect_clicked => RatingMsg::Click(2),
            },
            
            gtk4::Button {
                set_label: if model.effective_rating() >= 3 { "★" } else { "☆" },
                add_css_class: "flat",
                add_css_class: if model.effective_rating() >= 3 { "star-filled" } else { "star-empty" },
                connect_clicked => RatingMsg::Click(3),
            },
            
            gtk4::Button {
                set_label: if model.effective_rating() >= 4 { "★" } else { "☆" },
                add_css_class: "flat",
                add_css_class: if model.effective_rating() >= 4 { "star-filled" } else { "star-empty" },
                connect_clicked => RatingMsg::Click(4),
            },
            
            gtk4::Button {
                set_label: if model.effective_rating() >= 5 { "★" } else { "☆" },
                add_css_class: "flat",
                add_css_class: if model.effective_rating() >= 5 { "star-filled" } else { "star-empty" },
                connect_clicked => RatingMsg::Click(5),
            },
            
            // Clear button
            gtk4::Button {
                set_icon_name: "edit-clear-symbolic",
                set_tooltip_text: Some("Clear rating"),
                add_css_class: "flat",
                add_css_class: "circular",
                set_margin_start: 4,
                connect_clicked => RatingMsg::Click(0),
            },
        }
    }

    fn init(
        init: Self::Init,
        root: Self::Root,
        _sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let model = RatingWidget {
            rating: init,
            hover_rating: None,
        };
        
        let widgets = view_output!();
        
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, sender: ComponentSender<Self>) {
        match msg {
            RatingMsg::SetRating(rating) => {
                self.rating = rating;
            }
            RatingMsg::HoverRating(rating) => {
                self.hover_rating = Some(rating);
            }
            RatingMsg::HoverLeave => {
                self.hover_rating = None;
            }
            RatingMsg::Click(rating) => {
                // Toggle: clicking the current rating clears it
                let new_rating = if self.rating == rating { 0 } else { rating };
                self.rating = new_rating;
                let _ = sender.output(RatingOutput::RatingChanged(new_rating));
            }
        }
    }
}

impl RatingWidget {
    fn effective_rating(&self) -> i32 {
        self.hover_rating.unwrap_or(self.rating)
    }
}
