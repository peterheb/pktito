package net.pktito.app

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner

/**
 * Which model is in use once Manage models deletes the one that was
 * (Models.next): another downloaded, the catalog model an own file stood
 * in for first, and none once none is here.
 */
@RunWith(RobolectricTestRunner::class)
class ModelsTest {
    private fun offer(name: String, have: Boolean) = Offer(
        name, name, listOf("en"), "", Part(File("$name.pkf"), 1, "", here = have), Part(File("$name-slim.pkf"), 1, "", here = false), null,
        Edition.CPU, if (have) Edition.CPU else null, 0,
    )

    @Test fun theModelInUseGivesWayToAnother() =
        assertEquals("v3", Models.next(listOf(offer("v2", true), offer("v3", true)), "v2", "v2")?.name)

    @Test fun theLastLeavesNone() =
        assertNull(Models.next(listOf(offer("v2", true), offer("v3", false)), "v2", "v2"))

    @Test fun anOwnFileGivesWayToTheModelItStoodFor() =
        assertEquals("v3", Models.next(listOf(offer("v2", true), offer("v3", true)), "v3", null)?.name)

    @Test fun anOwnFileWhoseModelIsGoneGivesWayToAny() =
        assertEquals("v2", Models.next(listOf(offer("v2", true), offer("v3", false)), "v3", null)?.name)

    @Test fun anOwnFileAloneLeavesNone() =
        assertNull(Models.next(listOf(offer("v2", false), offer("v3", false)), "v2", null))
}
