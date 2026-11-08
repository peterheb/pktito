package net.pktito.app

import java.io.File
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Which model is in use once Manage models deletes the one that was (Models.next): another
 * downloaded, the catalog model an own file stood in for first, and none once none is here. And the
 * core's view of a model as Kotlin reads it (Models.offer, editions.rs View): what runs, the offer,
 * a refusal with nothing left to run, and the whole file among its files. Which edition fits which
 * chip is the core's, and its tests' (editions.rs).
 */
@RunWith(RobolectricTestRunner::class)
class ModelsTest {
    private fun offer(name: String, have: Boolean) =
        Offer(
            name,
            name,
            listOf("en"),
            "",
            "cpu",
            if (have) Runs("cpu", File("$name.pkf"), null) else null,
            null,
            null,
            1,
            emptyList(),
            Part(File("$name.pkf"), 1, ""),
            taken = 0,
        )

    @Test
    fun theModelInUseGivesWayToAnother() =
        assertEquals(
            "v3",
            Models.next(listOf(offer("v2", true), offer("v3", true)), "v2", "v2")?.name,
        )

    @Test
    fun theLastLeavesNone() =
        assertNull(Models.next(listOf(offer("v2", true), offer("v3", false)), "v2", "v2"))

    @Test
    fun anOwnFileGivesWayToTheModelItStoodFor() =
        assertEquals(
            "v3",
            Models.next(listOf(offer("v2", true), offer("v3", true)), "v3", null)?.name,
        )

    @Test
    fun anOwnFileWhoseModelIsGoneGivesWayToAny() =
        assertEquals(
            "v2",
            Models.next(listOf(offer("v2", true), offer("v3", false)), "v3", null)?.name,
        )

    @Test
    fun anOwnFileAloneLeavesNone() =
        assertNull(Models.next(listOf(offer("v2", false), offer("v3", false)), "v2", null))

    private val dir = File("/models")

    /**
     * The English model as the core's view gives it (editions.rs, View, through Engine.catalog):
     * its CPU's edition and the S24's, with what runs, why the NPU refused and the offer.
     */
    private fun view(runs: String?, refused: String?, offer: String?) =
        Models.offer(
            dir,
            JSONObject(
                """
                {"name": "parakeet-tdt-0.6b-v2", "key": "english", "label": "English",
                 "about": "", "languages": ["en"], "accuracy": "",
                 "editions": [
                   {"id": "cpu", "runtime": {"kind": "cpu"},
                    "model": {"file": "v2.pkf", "bytes": 624480196, "sha256": "8bfa"},
                    "encoder": null},
                   {"id": "qnn-sm8650",
                    "runtime": {"kind": "qnn", "chip": "57,75,8,1,0", "systems": ["android"]},
                    "model": {"file": "v2-slim.pkf", "bytes": 12115524, "sha256": "a1"},
                    "encoder": {"file": "v2-sm8650.qnn", "bytes": 714506856, "sha256": "40a5"}}
                 ],
                 "chosen": "qnn-sm8650", "runs": ${runs ?: "null"},
                 "refused": ${refused?.let { "\"$it\"" } ?: "null"},
                 "offer": ${offer ?: "null"}, "bytes": 726622380}
                """
            ),
        )

    @Test
    fun theNpusEditionRunsFromTheSlimFileWithItsGraph() {
        val o =
            view(
                """{"edition": "qnn-sm8650", "model": "/models/v2-slim.pkf",
                    "encoder": "/models/v2-sm8650.qnn"}""",
                null,
                null,
            )
        assertTrue(o.have)
        assertEquals("v2-slim.pkf", o.runs?.model?.name)
        assertEquals("v2-sm8650.qnn", o.runs?.graph?.name)
        assertTrue(o.runs!!.npu)
        assertEquals(0L, o.faster)
        assertEquals(0L, o.update)
    }

    @Test
    fun theCpusEditionRunsWithNoGraph() {
        val o =
            view(
                """{"edition": "cpu", "model": "/models/v2.pkf", "encoder": null}""",
                null,
                """{"kind": "faster", "bytes": 726622380, "kept": 0,
                    "files": ["v2-slim.pkf", "v2-sm8650.qnn"]}""",
            )
        assertNull(o.runs?.graph)
        assertFalse(o.runs!!.npu)
        assertEquals(726_622_380L, o.faster)
        assertEquals(listOf("v2-slim.pkf", "v2-sm8650.qnn"), o.get?.files)
    }

    @Test
    fun anUpdateIsItsOwnOffer() {
        val o =
            view(
                """{"edition": "qnn-sm8650", "model": "/models/v2-slim.pkf",
                    "encoder": "/models/v2-sm8650.qnn"}""",
                null,
                """{"kind": "update", "bytes": 714506856, "kept": 1000,
                    "files": ["v2-sm8650.qnn"]}""",
            )
        assertEquals(714_506_856L, o.update)
        assertEquals(0L, o.faster)
        assertEquals(0L, o.partial)
        assertEquals(1000L, o.get?.kept)
    }

    @Test
    fun aRefusalWithNothingLeftToRunSaysWhy() {
        val o =
            view(
                null,
                "graphExecute 6033",
                """{"kind": "download", "bytes": 624480196, "kept": 0, "files": ["v2.pkf"]}""",
            )
        assertFalse(o.have)
        assertEquals("graphExecute 6033", o.refused)
        assertEquals(624_480_196L, o.bytes)
    }

    @Test
    fun aModelNeverDownloadedHasNoRefusal() {
        val o =
            view(
                null,
                null,
                """{"kind": "download", "bytes": 726622380, "kept": 5,
                    "files": ["v2-slim.pkf", "v2-sm8650.qnn"]}""",
            )
        assertNull(o.refused)
        assertEquals(726_622_380L, o.bytes)
        assertEquals(5L, o.partial)
    }

    @Test
    fun itsFilesAreEveryEditionsAndTheWholeOneIsTheCpus() {
        val o = view(null, null, null)
        assertEquals(listOf("v2.pkf", "v2-slim.pkf", "v2-sm8650.qnn"), o.files.map { it.name })
        assertEquals(File(dir, "v2.pkf"), o.whole.file)
        assertEquals("8bfa", o.whole.sha256)
    }
}
