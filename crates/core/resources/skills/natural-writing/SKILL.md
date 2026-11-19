---
name: natural-writing
description: Rewrites AI-generated content into natural, concise, professional Indonesian while preserving requirements, technical accuracy, and identifiers. Use when the user asks to naturalize, humanize, rewrite, or improve the readability of AI-generated technical or Jira content in Indonesian.
---

# Naturalize AI Output

## Purpose

Rewrite AI-generated content so it sounds natural, clear, and human-written while preserving the original meaning, technical accuracy, and important details.

The final output MUST be written in natural Indonesian.

## Language Requirement

**The final output must always be in Indonesian.**

Use Indonesian as the primary language for all explanations, descriptions, objectives, requirements, and instructions.

English is allowed only for:

* Technical terms that are commonly used in software engineering.
* Function, variable, component, class, or file names.
* API endpoints.
* Jira ticket keys.
* Product, feature, or system names.
* Code and code blocks.
* Existing technical identifiers that must remain unchanged.

Do not translate technical identifiers.

Examples:

* `formatCodAmountIntl` → keep unchanged
* `POST /horde/v2/tariff/estimation` → keep unchanged
* `GENESIS-20787` → keep unchanged
* `create-booking-manual.vue` → keep unchanged
* "sticky footer" → can remain in English if that is the terminology commonly used by the engineering team

## Natural Indonesian Style

The output should sound like it was written naturally by a software engineer or product team member in Indonesia.

Prefer:

* Bahasa Indonesia yang sederhana dan langsung.
* Kalimat yang tidak terlalu panjang.
* Istilah teknis yang umum digunakan oleh developer.
* Gaya profesional tetapi tidak kaku.
* Struktur yang mudah dibaca.
* Penjelasan yang praktis dan to the point.

Avoid:

* Bahasa Indonesia yang terlalu formal.
* Terjemahan literal dari bahasa Inggris.
* Kalimat yang terdengar seperti hasil terjemahan mesin.
* Penggunaan kata yang berlebihan.
* Repetitive sentence patterns.
* AI-like filler phrases.

## Avoid AI-like Phrases

Avoid unnecessary phrases such as:

* "Penting untuk dicatat bahwa..."
* "Dalam rangka..."
* "Selain itu..."
* "Selanjutnya..."
* "Dengan demikian..."
* "Hal ini bertujuan untuk..."
* "Hal ini memastikan bahwa..."
* "Perlu diperhatikan bahwa..."
* "Secara keseluruhan..."
* "Implementasi yang robust..."
* "solusi yang optimal"
* "secara seamless"
* "pendekatan yang komprehensif"

Replace them with simpler Indonesian when possible.

For example:

Instead of:

> Hal ini bertujuan untuk memastikan bahwa user dapat melakukan proses submit dengan baik.

Use:

> Dengan validasi ini, user hanya bisa submit jika semua field wajib sudah terisi.

## Preserve Meaning

Do not:

* Change the original requirement.
* Add new requirements.
* Remove important technical details.
* Make assumptions that are not present in the source.
* Change business rules.
* Change technical behavior.

Only improve the wording and readability.

## Preserve Technical Details

Keep the following exactly as they appear:

* Jira keys
* Function names
* Variable names
* Component names
* API endpoints
* File paths
* Code
* Technical identifiers
* Error messages
* Database fields
* Configuration keys

Do not modify or translate them unless explicitly instructed.

## Preserve YAML Frontmatter

If the source document starts with YAML frontmatter, preserve the entire block exactly as it is.

* Do not change, add, remove, reorder, translate, or reformat frontmatter keys and values.
* Preserve the opening and closing `---` delimiters.
* Naturalize only the Markdown body after the closing `---` delimiter.
* Do not add frontmatter to a document that does not already have it unless explicitly instructed.

This includes Jira detailing metadata such as `issue`, `story`, `epic`, `stack`, `status`, `sources`, `dependencies`, and `artifact_path`.

## Context: Jira / Engineering Tasks

When the output is intended for a Jira task:

* Use natural Indonesian.
* Keep technical terminology commonly used by developers.
* Make the Objective concise.
* Make requirements explicit.
* Avoid unnecessary background explanation.
* Preserve implementation details.
* Do not make the task sound like a formal business document.

Example:

AI-generated:

> Tujuan dari task ini adalah melakukan implementasi perubahan pada mekanisme validasi sehingga user dapat melakukan submit apabila seluruh mandatory field telah terisi dengan valid.

Naturalized:

> ## Objective
>
> Perbaiki validasi agar user hanya bisa submit ketika semua mandatory field sudah terisi dengan valid.

## Output Rules

1. Always output the final content in Indonesian.
2. Preserve technical terms and identifiers.
3. Preserve the original meaning.
4. Remove unnecessary verbosity.
5. Make sentences sound natural and human-written.
6. Keep the original structure when it is useful.
7. Do not add explanations about the rewriting process.
8. Return only the rewritten content unless the user explicitly asks for an explanation.

## Final Quality Check

Before returning the result, verify:

* Is the entire explanation written in Indonesian?
* Does it sound natural to an Indonesian software engineer?
* Does it avoid overly formal or robotic language?
* Are technical terms and identifiers preserved?
* Has the original meaning remained unchanged?
* Has unnecessary verbosity been removed?
* Does it still clearly communicate the requirement?

The target style is:

**Natural + Professional + Concise + Technical**

The output should feel like it was written by a real Indonesian engineer, not translated from English and not generated by an AI.
