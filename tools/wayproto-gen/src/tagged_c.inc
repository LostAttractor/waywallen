
static void tagged_patch_u32(ww_buf_t *b, size_t pos, uint32_t value) {
    for (size_t i = 0; i < 4; ++i) b->data[pos + i] = (uint8_t)(value >> (8 * i));
}

typedef struct {
    ww_rd_t body;
    uint32_t tags[256];
    size_t count;
} ww_tagged_reader_t;

static int tagged_begin(ww_rd_t *r, ww_tagged_reader_t *out) {
    uint32_t revision, len;
    int rc = rd_u32(r, &revision);
    if (rc) return rc;
    if ((rc = rd_u32(r, &len))) return rc;
    if (len > 65520 || len % 4 != 0) return WW_ERR_BAD_ARRAY;
    if ((rc = rd_need(r, len))) return rc;
    out->body = (ww_rd_t){r->buf + r->pos, 0, revision == 1 ? len : 0};
    out->count = 0;
    r->pos += len;
    return WW_OK;
}

static int tagged_next(ww_tagged_reader_t *r, uint32_t *tag, uint32_t *kind, ww_rd_t *value) {
    uint32_t len;
    int rc;
    if (r->count == 256) return WW_ERR_BAD_ARRAY;
    if ((rc = rd_u32(&r->body, tag))) return rc;
    if ((rc = rd_u32(&r->body, kind))) return rc;
    if ((rc = rd_u32(&r->body, &len))) return rc;
    if (*tag == 0 || len % 4 != 0) return WW_ERR_BAD_ARRAY;
    for (size_t i = 0; i < r->count; ++i) {
        if (r->tags[i] == *tag) return WW_ERR_BAD_ARRAY;
    }
    r->tags[r->count++] = *tag;
    if ((rc = rd_need(&r->body, len))) return rc;
    *value = (ww_rd_t){r->body.buf + r->body.pos, 0, len};
    r->body.pos += len;
    return WW_OK;
}

static bool tagged_utf8(const uint8_t *s, size_t len) {
    for (size_t i = 0; i < len;) {
        uint32_t c = s[i++];
        if (c < 0x80) continue;
        size_t n;
        uint32_t minimum;
        if (c >= 0xc2 && c <= 0xdf) { n = 1; minimum = 0x80; c &= 0x1f; }
        else if (c >= 0xe0 && c <= 0xef) { n = 2; minimum = 0x800; c &= 0x0f; }
        else if (c >= 0xf0 && c <= 0xf4) { n = 3; minimum = 0x10000; c &= 7; }
        else return false;
        if (n > len - i) return false;
        while (n--) {
            if ((s[i] & 0xc0) != 0x80) return false;
            c = (c << 6) | (s[i++] & 0x3f);
        }
        if (c < minimum || c > 0x10ffff || (c >= 0xd800 && c <= 0xdfff)) return false;
    }
    return true;
}

static int tagged_validate_value(uint32_t kind, ww_rd_t *r) {
    uint32_t len;
    int rc;
    if (kind >= 0x100 || kind == 9) {
        if ((rc = rd_u32(r, &len))) return rc;
        if (len > 1024) return WW_ERR_BAD_ARRAY;
        for (uint32_t i = 0; i < len; ++i) {
            if ((rc = tagged_validate_value(kind == 9 ? 8 : kind - 0x100, r))) return rc;
            if (kind == 9 && (rc = tagged_validate_value(8, r))) return rc;
        }
        return WW_OK;
    }
    if (kind == 8) {
        if ((rc = rd_u32(r, &len))) return rc;
        if (len == 0 || len > 4097) return WW_ERR_BAD_STRING;
        size_t padded = ((size_t)len + 3) & ~(size_t)3;
        if ((rc = rd_need(r, padded))) return rc;
        const uint8_t *s = r->buf + r->pos;
        if (memchr(s, 0, len - 1) || !tagged_utf8(s, len - 1)) return WW_ERR_BAD_STRING;
        for (size_t i = len - 1; i < padded; ++i) if (s[i]) return WW_ERR_BAD_STRING;
        r->pos += padded;
        return WW_OK;
    }
    if (kind == 11) {
        uint32_t revision;
        if ((rc = rd_u32(r, &revision))) return rc;
        if ((rc = rd_u32(r, &len))) return rc;
    } else len = (kind == 4 || kind == 5 || kind == 7) ? 8 : (kind == 10 ? 16 : 4);
    if ((rc = rd_need(r, len))) return rc;
    r->pos += len;
    return WW_OK;
}

static int tagged_validate(uint32_t kind, ww_rd_t value) {
    return tagged_validate_value(kind, &value);
}
