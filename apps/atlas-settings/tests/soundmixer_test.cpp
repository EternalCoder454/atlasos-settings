#include "soundmixer.h"

#include <PulseAudioQt/Context>
#include <PulseAudioQt/Sink>
#include <PulseAudioQt/Source>

#include <QGuiApplication>
#include <QSignalSpy>
#include <QTest>

using namespace Qt::StringLiterals;

// Against a sound server that is already running (PipeWire's PulseAudio,
// started on a private runtime directory by the test run: never the real
// one): skipped when there is none, as on a build machine. Changes the
// volume and mute of whatever sinks and sources it finds, so it must never
// be pointed at a real session.
class SoundMixerTest : public QObject
{
    Q_OBJECT

    std::unique_ptr<SoundMixer> m_mixer;

    static QStringList names(QObject *list)
    {
        auto *model = qobject_cast<QAbstractItemModel *>(list);
        QStringList out;
        const int role = model->roleNames().key("label", -1);
        for (int i = 0; i < model->rowCount(); ++i) {
            out << model->data(model->index(i, 0), role).toString();
        }
        return out;
    }

private Q_SLOTS:
    void initTestCase()
    {
        if (qEnvironmentVariable("ATLAS_TEST_SOUND") != u"private"_s) {
            QSKIP("needs a private sound server (ATLAS_TEST_SOUND=private)");
        }
        m_mixer = std::make_unique<SoundMixer>();
        QVERIFY2(QTest::qWaitFor([&] { return m_mixer->ready() && m_mixer->defaultOutput() != nullptr; }, 8000), "no sound server answered");
        QTest::qWait(300);
    }

    void cleanupTestCase()
    {
        m_mixer.reset();
    }

    void listsOutputsAndInputsWithoutMonitors()
    {
        const QStringList outputs = names(m_mixer->outputs());
        const QStringList inputs = names(m_mixer->inputs());
        QVERIFY2(outputs.contains(u"Built-in Audio"_s), qPrintable(outputs.join(u'|')));
        QVERIFY2(outputs.contains(u"Test HDMI Output"_s), qPrintable(outputs.join(u'|')));
        QVERIFY2(inputs.contains(u"Test Microphone"_s), qPrintable(inputs.join(u'|')));
        for (const QString &i : inputs) {
            QVERIFY2(!i.startsWith(u"Monitor of"), qPrintable(i));
        }
        QCOMPARE(m_mixer->property("revision").toInt() >= 0, true);
    }

    void choicesNameTheDevice()
    {
        const QVariantList choices = m_mixer->outputChoices();
        QVERIFY(choices.size() >= 2);
        for (const QVariant &c : choices) {
            const QVariantMap m = c.toMap();
            QVERIFY(!m[u"title"_s].toString().isEmpty());
            QVERIFY(!m[u"value"_s].toString().isEmpty());
        }
    }

    void theDefaultOutputsVolumeAndMuteAreLive()
    {
        QObject *sink = m_mixer->defaultOutput();
        QVERIFY(sink);
        const qint64 normal = m_mixer->normalVolume();
        QSignalSpy volume(sink, SIGNAL(volumeChanged()));
        QVERIFY(sink->setProperty("volume", normal / 4));
        QVERIFY(QTest::qWaitFor([&] { return m_mixer->percent(sink->property("volume").toLongLong()) == 25; }, 3000));
        QVERIFY(volume.count() >= 1);
        QSignalSpy muted(sink, SIGNAL(mutedChanged()));
        sink->setProperty("muted", true);
        QVERIFY(QTest::qWaitFor([&] { return sink->property("muted").toBool(); }, 3000));
        sink->setProperty("muted", false);
        QVERIFY(QTest::qWaitFor([&] { return !sink->property("muted").toBool(); }, 3000));
        QVERIFY(muted.count() >= 2);
    }

    void theDefaultCanBeChosenByNameOnlyFromTheList()
    {
        const QVariantList choices = m_mixer->outputChoices();
        QString other;
        const QString now = m_mixer->defaultOutput()->property("name").toString();
        for (const QVariant &c : choices) {
            if (c.toMap()[u"value"_s].toString() != now) {
                other = c.toMap()[u"value"_s].toString();
            }
        }
        QVERIFY(!other.isEmpty());
        QVERIFY(!m_mixer->setDefaultOutput(u"no-such-device"_s));
        QVERIFY(!m_mixer->setDefaultOutput(QString()));
        QSignalSpy changed(m_mixer.get(), &SoundMixer::defaultsChanged);
        QVERIFY(m_mixer->setDefaultOutput(other));
        QVERIFY(QTest::qWaitFor([&] { return m_mixer->defaultOutput() && m_mixer->defaultOutput()->property("name").toString() == other; }, 3000));
        QVERIFY(changed.count() >= 1);
        // and back
        QVERIFY(m_mixer->setDefaultOutput(now));
        QVERIFY(QTest::qWaitFor([&] { return m_mixer->defaultOutput()->property("name").toString() == now; }, 3000));
    }

    void inputsHaveVolumeAndMuteToo()
    {
        QObject *source = m_mixer->defaultInput();
        QVERIFY(source);
        source->setProperty("volume", m_mixer->normalVolume() / 2);
        QVERIFY(QTest::qWaitFor([&] { return m_mixer->percent(source->property("volume").toLongLong()) == 50; }, 3000));
        source->setProperty("muted", true);
        QVERIFY(QTest::qWaitFor([&] { return source->property("muted").toBool(); }, 3000));
        source->setProperty("muted", false);
    }

    void appsAreListedWhileTheyPlay()
    {
        // tests/run-sound-test.sh starts a stream named "Test Player" before this.
        if (qEnvironmentVariable("ATLAS_TEST_SOUND_STREAM") != u"1"_s) {
            QSKIP("no test stream");
        }
        QVERIFY2(QTest::qWaitFor([&] { return names(m_mixer->apps()).contains(u"Test Player"_s); }, 5000), qPrintable(names(m_mixer->apps()).join(u'|')));
    }
};

QTEST_MAIN(SoundMixerTest)
#include "soundmixer_test.moc"
