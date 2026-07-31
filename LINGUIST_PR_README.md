# LYZARD Linguist PR Preparation

## languages.yml entry

Add this to `lib/linguist/languages.yml` (alphabetically between L and M):

```yaml
LYZARD:
  type: programming
  color: "#BCE8C5"
  extensions:
  - ".lyz"
  tm_scope: source.lyz
  ace_mode: text
  language_id: <RUN_script/update-ids_TO_GENERATE_THIS>
```

## PR Checklist

- [ ] Add entry to `lib/linguist/languages.yml`
- [ ] Run `script/add-grammar https://github.com/lyzard-lang/lyzard-grammar`
- [ ] Run `script/update-ids` to generate language_id
- [ ] Add samples to `samples/LYZARD/*.lyz`
- [ ] Open PR with search results link showing >= 2000 `.lyz` files on GitHub
- [ ] Use the PR template and fill in ALL fields

## Grammar repo

The grammar repo `lyzard-lang/lyzard-grammar` must be public and contain:
- `lyzard.tmLanguage.json` (the TextMate grammar)
- `LICENSE` (MIT — approved by linguist)
- `README.md`

## Blocking issue

**PR #8090 was rejected because LYZARD has 0 repositories on GitHub using `.lyz` files.**
Linguist requires >= 2000 files (or >= 200 for singleton extensions) across unique `:user/:repo` combos.
