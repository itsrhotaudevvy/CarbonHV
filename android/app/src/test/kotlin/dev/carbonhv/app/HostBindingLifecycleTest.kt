package dev.carbonhv.app

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class HostBindingLifecycleTest {
    private class Fake(var bindResult: Boolean = true, var throwOnBind: Boolean = false) {
        var binds = 0
        var unbinds = 0
        val lifecycle = HostBindingLifecycle(
            bind = {
                binds++
                if (throwOnBind) throw SecurityException("denied")
                bindResult
            },
            unbind = { unbinds++ },
        )
    }

    @Test
    fun failed_bind_on_stock_builds_is_still_released_on_destroy() {
        val fake = Fake(bindResult = false)
        fake.lifecycle.bindIfNeeded()
        assertFalse(fake.lifecycle.connected)
        assertTrue(fake.lifecycle.registered)

        fake.lifecycle.release()
        assertEquals(1, fake.unbinds)
        assertFalse(fake.lifecycle.registered)
    }

    @Test
    fun security_exception_registers_nothing_and_never_unbinds() {
        val fake = Fake(throwOnBind = true)
        fake.lifecycle.bindIfNeeded()
        fake.lifecycle.release()
        assertEquals(0, fake.unbinds)
    }

    @Test
    fun binding_is_idempotent_and_release_happens_once() {
        val fake = Fake()
        fake.lifecycle.bindIfNeeded()
        fake.lifecycle.bindIfNeeded()
        assertEquals(1, fake.binds)
        assertTrue(fake.lifecycle.connected)
        fake.lifecycle.release()
        fake.lifecycle.release()
        assertEquals(1, fake.unbinds)
    }

    @Test
    fun binding_death_unbinds_before_rebinding() {
        val fake = Fake()
        fake.lifecycle.bindIfNeeded()
        fake.lifecycle.rebindAfterDeath()
        assertEquals(1, fake.unbinds)
        assertEquals(2, fake.binds)
        assertTrue(fake.lifecycle.registered)
    }

    @Test
    fun framework_already_unbound_is_tolerated() {
        val lifecycle = HostBindingLifecycle(bind = { true }, unbind = { throw IllegalArgumentException() })
        lifecycle.bindIfNeeded()
        lifecycle.release()
        assertFalse(lifecycle.registered)
    }
}
