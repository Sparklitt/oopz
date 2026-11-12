// Generated from oopz's Rust enums (crates/oopz-export/src/bindings.rs): change them, not this file.

/** The apps oopz reads (oopz-model's App). */
export type AppCode = "whatsapp" | "telegram" | "sms" | "line" | "viber" | "signal" | "imessage";

/** What a message is: msgkinds' kinds (oopz-model's Kind). */
export type Kind = "text" | "link" | "photo" | "video" | "video_note" | "gif" | "voice" | "audio" | "sticker" | "document" | "location" | "contact" | "poll" | "call" | "album" | "sticker_pack" | "event" | "payment" | "game" | "business" | "story" | "reaction" | "system" | "deleted" | "unknown";

/** The kinds that come with a file (oopz-model's MediaKind). */
export type MediaKind = "photo" | "video" | "video_note" | "gif" | "voice" | "audio" | "sticker" | "document";

/** The kinds a card part holds (oopz-model's CardKind). */
export type CardKind = "album" | "sticker_pack" | "event" | "payment" | "game" | "business" | "story" | "reaction";

/** What an app's notice says happened (oopz-model's Event). */
export type EventCode = "encryption" | "security_code_changed" | "number_changed" | "device_changed" | "account_deleted" | "contact_joined" | "business_notice" | "group_created" | "topic_created" | "renamed" | "description_changed" | "picture_changed" | "picture_removed" | "settings_changed" | "member_added" | "member_joined" | "member_left" | "member_removed" | "join_requested" | "admin_added" | "admin_removed" | "admin_changed" | "owner_changed" | "community_changed" | "invite_link_reset" | "disappearing" | "kept_in_chat" | "chat_locked" | "screenshot" | "pinned" | "unpinned" | "blocked" | "unblocked" | "message_deleted" | "history_cleared" | "group_call" | "poll_closed" | "payment_status" | "missed_messages" | "theme_changed" | "profile_changed" | "other";

/** A library export's format (oopz-export's Format, and the PDF). */
export type ExportFormat = "html" | "json" | "csv" | "txt" | "vcard" | "media-zip" | "pdf";

