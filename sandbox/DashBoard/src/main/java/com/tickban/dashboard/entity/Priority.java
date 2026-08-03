package com.tickban.dashboard.entity;

import com.fasterxml.jackson.annotation.JsonCreator;
import com.fasterxml.jackson.annotation.JsonValue;

public enum Priority {
    LOW((byte) 1),
    MEDIUM((byte) 2),
    HIGH((byte) 3);

    private final byte value;

    Priority(byte value) {
        this.value = value;
    }

    // ✅ 修复：加上 mode = DELEGATING
    @JsonCreator(mode = JsonCreator.Mode.DELEGATING)
    public static Priority fromString(String name) {
        if (name == null || name.isEmpty()) return null;
        try {
            return Priority.valueOf(name.trim().toUpperCase());
        } catch (IllegalArgumentException e) {
            throw new IllegalArgumentException("Invalid priority: " + name + ". Use: low, medium, high");
        }
    }

    @JsonValue
    @Override
    public String toString() {
        return this.name().toLowerCase();
    }

    public byte getValue() {
        return value;
    }

    public static Priority fromByte(Byte b) {
        if (b == null) return null;
        return switch (b) {
            case 1 -> LOW;
            case 2 -> MEDIUM;
            case 3 -> HIGH;
            default -> throw new IllegalArgumentException("Invalid priority byte: " + b);
        };
    }
}