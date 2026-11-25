package app.quest.android.focus

import app.quest.android.Samples
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class ServiceDecisionTest {
    private val running = Samples.running
    private val other = running.copy(taskId = "t4", title = "Edit chapter 2")
    private val noTimer = TimerReading(null)

    private fun decide(shown: ShownNotification, session: uniffi.quest_mobile.RunningSession?) = decideService(shown, TimerReading(session))

    @Test
    fun aTimerThatStartsStartsTheService() {
        assertEquals(ServiceDecision.Start(running), decide(ShownNotification.None, running))
    }

    @Test
    fun theSameTimerChangesNothing() {
        assertEquals(ServiceDecision.Nothing, decide(ShownNotification.Running(running), running))
        assertEquals(ServiceDecision.Nothing, decide(ShownNotification.Fallback(running), running))
    }

    @Test
    fun anotherTaskOrTitleUpdatesTheNotification() {
        assertEquals(ServiceDecision.Update(other), decide(ShownNotification.Running(running), other))
        val renamed = running.copy(title = "Outline chapter three")
        assertEquals(ServiceDecision.Update(renamed), decide(ShownNotification.Running(running), renamed))
        assertEquals(ServiceDecision.Update(other), decide(ShownNotification.Fallback(running), other))
    }

    @Test
    fun noTimerStopsTheServiceAndRemovesAPlainNotification() {
        // Paused in the app, completed, or ended on another device.
        assertEquals(ServiceDecision.Stop, decide(ShownNotification.Running(running), null))
        assertEquals(ServiceDecision.Stop, decide(ShownNotification.Fallback(running), null))
        assertEquals(ServiceDecision.Nothing, decide(ShownNotification.None, null))
    }

    @Test
    fun afterAProcessStartALeftoverNotificationGoesUnlessATimerRuns() {
        assertEquals(ServiceDecision.Stop, decideService(ShownNotification.Unknown, noTimer))
        assertEquals(ServiceDecision.Start(running), decide(ShownNotification.Unknown, running))
    }

    @Test
    fun aPausedNotificationStaysWhileItsTaskIsOpenAndNoTimerRuns() {
        val paused = ShownNotification.Paused(running)
        assertEquals(ServiceDecision.Nothing, decideService(paused, TimerReading(null, pausedTaskOpen = true)))
        assertEquals(ServiceDecision.Start(other), decide(paused, other))
    }

    @Test
    fun aPausedNotificationGoesWhenItsTaskIsDoneOrDeleted() {
        val paused = ShownNotification.Paused(running)
        assertEquals(ServiceDecision.Stop, decideService(paused, TimerReading(null, pausedTaskOpen = false)))
    }

    @Test
    fun onlyAPlainNotificationRetriesTheService() {
        assertEquals(ServiceDecision.Start(running), ShownNotification.Fallback(running).retryService())
        assertEquals(ServiceDecision.Nothing, ShownNotification.Running(running).retryService())
        assertEquals(ServiceDecision.Nothing, ShownNotification.Paused(running).retryService())
        assertEquals(ServiceDecision.Nothing, ShownNotification.None.retryService())
    }

    @Test
    fun theShownNotificationFollowsTheDecision() {
        assertEquals(ShownNotification.Running(running), ShownNotification.None.after(ServiceDecision.Start(running)))
        assertEquals(ShownNotification.Running(other), ShownNotification.Running(running).after(ServiceDecision.Update(other)))
        assertEquals(ShownNotification.Fallback(other), ShownNotification.Fallback(running).after(ServiceDecision.Update(other)))
        assertEquals(ShownNotification.None, ShownNotification.Running(running).after(ServiceDecision.Stop))
        assertEquals(ShownNotification.Paused(running), ShownNotification.Paused(running).after(ServiceDecision.Nothing))
    }

    @Test
    fun runningContentHasTheChronometerPauseAndComplete() {
        val content = runningContent(running)

        assertEquals("Outline chapter 3", content.title)
        assertEquals("Write the book", content.questName)
        assertEquals(running.chronometerBaseMs, content.chronometerBaseMs)
        assertEquals(true, content.ongoing)
        assertEquals(listOf(NotificationAction.Pause, NotificationAction.Complete), content.actions)
        assertEquals(content, ShownNotification.Fallback(running).content())
    }

    @Test
    fun pausedContentHasNoChronometerAndIsDismissible() {
        val content = pausedContent(running.copy(questName = null, questColorHex = null))

        assertNull(content.chronometerBaseMs)
        assertNull(content.questName)
        assertEquals(false, content.ongoing)
        assertEquals(false, content.running)
        assertEquals(listOf(NotificationAction.Resume, NotificationAction.Complete), content.actions)
        assertNull(ShownNotification.None.content())
        assertNull(ShownNotification.Unknown.content())
    }
}
