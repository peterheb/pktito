package net.pktito.app

import java.io.File
import java.nio.file.Files
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * The recent list from before it was backed up, moved from no_backup to the app's files once
 * (Recents.moveList): its index and transcripts with it, and a list already there, as a restore
 * leaves one, kept.
 */
@RunWith(RobolectricTestRunner::class)
class RecentsTest {
    private val root = Files.createTempDirectory("recents").toFile()
    private val old = File(root, "no_backup/recent")
    private val new = File(root, "files/recent")

    private fun list(dir: File, vararg ids: String) {
        dir.mkdirs()
        File(dir, "index.json").writeText(ids.joinToString(",", "[", "]") { "\"$it\"" })
        for (id in ids) File(dir, "$id.json").writeText("{}")
    }

    @Test
    fun theListMovesWithItsTranscripts() {
        list(old, "a", "b")
        new.parentFile!!.mkdirs()
        Recents.moveList(old, new)
        assertFalse(old.exists())
        assertEquals(setOf("index.json", "a.json", "b.json"), new.list()!!.toSet())
    }

    @Test
    fun aListAlreadyThereStays() {
        list(old, "a")
        list(new, "b")
        Recents.moveList(old, new)
        assertEquals(setOf("index.json", "b.json"), new.list()!!.toSet())
        assertTrue(File(old, "a.json").isFile)
    }

    @Test
    fun noListMovesNothing() {
        Recents.moveList(old, new)
        assertFalse(new.exists())
    }
}
