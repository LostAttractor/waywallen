module;
#include "waywallen/plugin_translation.moc.h"

module waywallen;
import qextra;
import :plugin_translation;
import :app;
import :backend;
import :notify;
import :proto;
import rstd;
import waywallen.i18n.po;

using namespace Qt::Literals::StringLiterals;
using namespace qextra::prelude;

namespace proto       = waywallen::control::v1;
namespace plugin_i18n = waywallen::i18n::po;

namespace waywallen
{
namespace
{

auto canonicalLocale(QString locale) -> QString {
    locale.replace(u'_', u'-');
    const QLocale parsed(locale);
    if (parsed.language() == QLocale::C) return locale;
    return parsed.bcp47Name();
}

auto localeFallbacks(const QString& locale) -> QStringList {
    auto        current = canonicalLocale(locale);
    QStringList result;
    while (! current.isEmpty()) {
        result.append(current);
        const auto separator = current.lastIndexOf(u'-');
        if (separator < 0) break;
        current.truncate(separator);
    }
    return result;
}

auto formatMessage(const QString& text, const QStringList& arguments) -> QString {
    if (arguments.isEmpty()) return text;
    QString result;
    result.reserve(text.size());
    for (qsizetype offset = 0; offset < text.size();) {
        if (text.at(offset) != u'%' || offset + 1 >= text.size() ||
            ! text.at(offset + 1).isDigit()) {
            result.append(text.at(offset++));
            continue;
        }
        auto end = offset + 1;
        while (end < text.size() && text.at(end).isDigit()) ++end;
        bool       valid = false;
        const auto index = QStringView(text).mid(offset + 1, end - offset - 1).toInt(&valid);
        if (valid && index > 0 && index <= arguments.size()) {
            result.append(arguments.at(index - 1));
        } else {
            result.append(QStringView(text).mid(offset, end - offset));
        }
        offset = end;
    }
    return result;
}

} // namespace

auto PluginTranslationStore::parseDocument(const QByteArray& po, const QString& expected_locale,
                                           MessageMap& messages, QString& error) -> bool {
    const auto bytes = rstd::slice<rstd::u8>::from_raw_parts(
        reinterpret_cast<const rstd::byte*>(po.constData()), rstd::usize(po.size()));
    auto text = rstd::str_::from_utf8(bytes);
    if (text.is_err()) {
        error = u"PO document is not valid UTF-8"_s;
        return false;
    }
    auto parsed = plugin_i18n::parse(rstd::move(text).unwrap_unchecked());
    if (parsed.is_err()) {
        const auto& value = parsed.unwrap_err();
        error = u"%1:%2: %3"_s.arg(value.line.to_primitive())
                    .arg(value.column.to_primitive())
                    .arg(QString::fromUtf8(reinterpret_cast<const char*>(value.message.data()),
                                           value.message.len().to_primitive()));
        return false;
    }
    const auto locale       = expected_locale.toUtf8();
    const auto locale_bytes = rstd::slice<rstd::u8>::from_raw_parts(
        reinterpret_cast<const rstd::byte*>(locale.constData()), rstd::usize(locale.size()));
    auto locale_text = rstd::str_::from_utf8(locale_bytes).unwrap();
    auto view        = plugin_i18n::runtime_view(locale_text, *parsed);
    if (view.is_err()) {
        const auto& value = view.unwrap_err();
        error = u"%1:%2: %3"_s.arg(value.line.to_primitive())
                    .arg(value.column.to_primitive())
                    .arg(QString::fromUtf8(reinterpret_cast<const char*>(value.message.data()),
                                           value.message.len().to_primitive()));
        return false;
    }
    for (const auto& translation : view->translations) {
        const auto msgid =
            QString::fromUtf8(reinterpret_cast<const char*>(translation.msgid.data()),
                              translation.msgid.len().to_primitive());
        const auto msgstr =
            QString::fromUtf8(reinterpret_cast<const char*>(translation.msgstr.data()),
                              translation.msgstr.len().to_primitive());
        messages.insert(msgid, msgstr);
    }
    return true;
}

auto PluginTranslationStore::instance() -> PluginTranslationStore* {
    static auto* store = new PluginTranslationStore(App::instance());
    return store;
}

PluginTranslationStore* PluginTranslationStore::create(QQmlEngine*, QJSEngine*) {
    auto* store = instance();
    QJSEngine::setObjectOwnership(store, QJSEngine::CppOwnership);
    return store;
}

PluginTranslationStore::PluginTranslationStore(QObject* parent): QObject(parent) {}
PluginTranslationStore::~PluginTranslationStore() = default;

void PluginTranslationStore::initialize() {
    if (m_initialized) return;
    m_initialized = true;
    auto* backend = App::instance()->backend();
    connect(backend, &Backend::connected, this, [this] {
        ++m_connection_generation;
        scheduleRefresh();
    });
    connect(backend, &Backend::disconnected, this, [this] {
        ++m_connection_generation;
        ++m_request_generation;
        clear();
    });
    connect(Notify::instance(), &Notify::pluginChanged, this, [this] {
        scheduleRefresh();
    });
}

void PluginTranslationStore::setLocale(const QString& locale) {
    const auto canonical = canonicalLocale(locale);
    if (canonical.isEmpty() || canonical == m_locale) return;
    m_locale = canonical;
    ++m_revision;
    Q_EMIT localeChanged();
    Q_EMIT revisionChanged();
}

auto PluginTranslationStore::translate(const QVariant& text) const -> QString {
    if (text.metaType().id() == QMetaType::QString) return text.toString();
    const auto value      = text.toMap();
    const auto plugin_id  = value.value(u"pluginId"_s).toString();
    const auto msgid      = value.value(u"msgid"_s).toString();
    const auto arguments  = value.value(u"arguments"_s).toStringList();
    auto       translated = msgid;
    if (! plugin_id.isEmpty() && ! msgid.isEmpty() &&
        QLocale(m_locale).language() != QLocale::English) {
        const auto plugin_it = m_translations.constFind(plugin_id);
        if (plugin_it != m_translations.constEnd()) {
            for (const auto& locale : localeFallbacks(m_locale)) {
                const auto locale_it = plugin_it->constFind(locale);
                if (locale_it == plugin_it->constEnd()) continue;
                const auto message_it = locale_it->constFind(msgid);
                if (message_it == locale_it->constEnd() || message_it->isEmpty()) continue;
                translated = *message_it;
                break;
            }
        }
    }
    return formatMessage(translated, arguments);
}

void PluginTranslationStore::scheduleRefresh() {
    if (m_refresh_scheduled) return;
    m_refresh_scheduled = true;
    QTimer::singleShot(0, this, [this] {
        m_refresh_scheduled = false;
        refresh();
    });
}

void PluginTranslationStore::refresh() {
    auto* backend = App::instance()->backend();
    auto  request = proto::Request {};
    request.setPluginTranslationList(proto::PluginTranslationListRequest {});
    const auto connection_generation = m_connection_generation;
    const auto request_generation    = ++m_request_generation;
    auto       self                  = QWatcher { this };
    m_requests.spawn([self,
                      backend,
                      request = std::move(request),
                      connection_generation,
                      request_generation]() mutable -> task<void> {
        auto result = co_await backend->send(std::move(request));
        if (! co_await QAsyncResult::qexecutor()) co_return;
        if (! self || connection_generation != self->m_connection_generation ||
            request_generation != self->m_request_generation)
            co_return;
        if (! result) {
            qWarning("plugin translations: refresh failed: %s",
                     qPrintable(result.unwrap_err_unchecked()));
            co_return;
        }

        auto          wire_response = result.unwrap_unchecked();
        const auto&   response      = wire_response.pluginTranslationList();
        PluginMap     translations;
        QSet<QString> invalid_plugins;
        QSet<QString> documents;
        for (const auto& document : response.documents()) {
            const auto plugin_id = document.pluginId();
            const auto locale    = canonicalLocale(document.locale());
            const auto key       = plugin_id + u'\n' + locale;
            if (plugin_id.isEmpty() || locale.isEmpty() || documents.contains(key)) {
                invalid_plugins.insert(plugin_id);
                qWarning("plugin translations: rejected duplicate or unnamed document for '%s'",
                         qPrintable(plugin_id));
                continue;
            }
            documents.insert(key);
            MessageMap messages;
            QString    error;
            if (! parseDocument(document.po(), document.locale(), messages, error)) {
                invalid_plugins.insert(plugin_id);
                qWarning("plugin translations: rejected %s/%s: %s",
                         qPrintable(plugin_id),
                         qPrintable(locale),
                         qPrintable(error));
                continue;
            }
            translations[plugin_id].insert(locale, std::move(messages));
        }
        for (const auto& plugin_id : invalid_plugins) translations.remove(plugin_id);
        self->replace(std::move(translations), response.generation());
        co_return;
    });
}

void PluginTranslationStore::clear() {
    if (m_translations.isEmpty() && m_generation == 0) return;
    m_translations.clear();
    m_generation = 0;
    ++m_revision;
    Q_EMIT revisionChanged();
}

void PluginTranslationStore::replace(PluginMap translations, quint64 generation) {
    m_translations = std::move(translations);
    m_generation   = generation;
    ++m_revision;
    Q_EMIT revisionChanged();
}

} // namespace waywallen

#include "waywallen/plugin_translation.moc.cpp"
