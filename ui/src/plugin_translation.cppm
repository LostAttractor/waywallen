module;
#include "QExtra/macro_qt.hpp"

#ifdef Q_MOC_RUN
#    include "waywallen/plugin_translation.moc"
#endif

export module waywallen:plugin_translation;
import qextra;
import rstd.cppstd;

export namespace waywallen
{

class PluginTranslationStore : public QObject {
    Q_OBJECT
    QML_NAMED_ELEMENT(PluginTranslations)
    QML_SINGLETON

    Q_PROPERTY(quint64 revision READ revision NOTIFY revisionChanged FINAL)
    Q_PROPERTY(QString locale READ locale NOTIFY localeChanged FINAL)

public:
    explicit PluginTranslationStore(QObject* parent);
    ~PluginTranslationStore() override;
    PluginTranslationStore() = delete;

    static auto                    instance() -> PluginTranslationStore*;
    static PluginTranslationStore* create(QQmlEngine*, QJSEngine*);

    auto revision() const -> quint64 { return m_revision; }
    auto locale() const -> const QString& { return m_locale; }
    void setLocale(const QString& locale);
    void initialize();

    Q_INVOKABLE QString translate(const QVariant& text) const;

Q_SIGNALS:
    void revisionChanged();
    void localeChanged();

private:
    using MessageMap = QHash<QString, QString>;
    using LocaleMap  = QHash<QString, MessageMap>;
    using PluginMap  = QHash<QString, LocaleMap>;

    static auto parseDocument(const QByteArray& po, const QString& expected_locale,
                              MessageMap& messages, QString& error) -> bool;

    void scheduleRefresh();
    void refresh();
    void clear();
    void replace(PluginMap translations, quint64 generation);

    PluginMap   m_translations;
    QString     m_locale { QString::fromLatin1("en") };
    quint64     m_generation { 0 };
    quint64     m_revision { 0 };
    quint64     m_connection_generation { 0 };
    quint64     m_request_generation { 0 };
    bool        m_initialized { false };
    bool        m_refresh_scheduled { false };
    QAsyncScope m_requests;
};

} // namespace waywallen
