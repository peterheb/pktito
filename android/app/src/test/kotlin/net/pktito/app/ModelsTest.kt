package net.pktito.app

import java.io.File
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Which model is in use once Manage models deletes the one that was (Models.next): another
 * downloaded, the catalog model an own file stood in for first, and none once none is here. Which
 * graph a chip gets (Models.graphFor): its own, else one of its Hexagon's that fits its VTCM, else
 * none. And what a model's files here load and what Faster fetches (Offer.file, fasterParts).
 */
@RunWith(RobolectricTestRunner::class)
class ModelsTest {
    private fun offer(name: String, have: Boolean) =
        Offer(
            name,
            name,
            listOf("en"),
            "",
            Part(File("$name.pkf"), 1, "", here = have),
            Part(File("$name-slim.pkf"), 1, "", here = false),
            null,
            Edition.CPU,
            if (have) Edition.CPU else null,
            0,
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

    // the catalog's chips (model.rs, Graph): the SM8650's, the SM8750's and the X1E80100's
    private val graphs =
        JSONArray(
            listOf("57,75,8,1,0", "69,79,8,1,0", "60,73,8,1,1").map { JSONObject().put("chip", it) }
        )

    private fun graphOf(chip: String?) = Models.graphFor(graphs, chip)?.getString("chip")

    @Test fun aChipGetsItsOwnGraph() = assertEquals("69,79,8,1,0", graphOf("69,79,8,1,0"))

    // QNN's SoC 43, the SM8550, a v73 with 8 MB
    @Test fun anotherSocGetsItsHexagonsGraph() = assertEquals("60,73,8,1,1", graphOf("43,73,8,1,0"))

    @Test fun lessVtcmThanTheGraphsGetsNone() = assertNull(graphOf("43,73,4,1,0"))

    // QNN's SoC 36, the SM8450, a v69
    @Test fun anotherHexagonGetsNone() = assertNull(graphOf("36,69,8,1,0"))

    @Test
    fun noChipGetsNone() {
        assertNull(graphOf(null))
        assertNull(graphOf(""))
        assertNull(graphOf("75"))
    }

    /**
     * The English model with these of its files here, its edition and Faster's bytes as
     * Models.catalog makes them.
     */
    private fun english(whole: Boolean, slim: Boolean, graph: Boolean): Offer {
        val usable = if (graph && (slim || whole)) Edition.NPU else if (whole) Edition.CPU else null
        return Offer(
            "v2",
            "english",
            listOf("en"),
            "",
            Part(File("v2.pkf"), 1, "", here = whole),
            Part(File("v2-slim.pkf"), 1, "", here = slim),
            Part(File("v2-sm8650.qnn"), 1, "", here = graph),
            Edition.NPU,
            usable,
            if (usable == Edition.CPU) listOf(slim, graph).count { !it }.toLong() else 0,
        )
    }

    @Test
    fun theNpusEditionLoadsTheSlimFile() {
        assertEquals("v2-slim.pkf", english(whole = true, slim = true, graph = true).file.name)
        assertEquals("v2-slim.pkf", english(whole = false, slim = true, graph = true).file.name)
    }

    // the whole file's encoder beside the graph, as Faster left it before the slim file came with
    // it
    @Test
    fun aGraphBesideTheWholeFileAloneLoadsTheWholeFile() =
        assertEquals("v2.pkf", english(whole = true, slim = false, graph = true).file.name)

    @Test
    fun theCpusEditionLoadsTheWholeFile() =
        assertEquals("v2.pkf", english(whole = true, slim = true, graph = false).file.name)

    @Test
    fun fasterFetchesTheNpusFilesNotHere() {
        assertEquals(
            listOf("v2-slim.pkf", "v2-sm8650.qnn"),
            english(whole = true, slim = false, graph = false).fasterParts.map { it.file.name },
        )
        assertEquals(
            listOf("v2-sm8650.qnn"),
            english(whole = true, slim = true, graph = false).fasterParts.map { it.file.name },
        )
        assertEquals(
            emptyList<String>(),
            english(whole = true, slim = true, graph = true).fasterParts.map { it.file.name },
        )
    }
}
