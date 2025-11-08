'use client';

import { useState, useEffect } from 'react';
import { useMailStore } from '@/store/mail-store';
import { jmapClient } from '@/lib/jmap-client';
import { useEditor, EditorContent } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import Underline from '@tiptap/extension-underline';
import Link from '@tiptap/extension-link';
import TextAlign from '@tiptap/extension-text-align';
import {
  XMarkIcon,
  PaperClipIcon,
  PaperAirplaneIcon,
} from '@heroicons/react/24/outline';
import {
  BoldIcon,
  ItalicIcon,
  UnderlineIcon,
  ListBulletIcon,
  ListNumberIcon,
  LinkIcon,
} from '@heroicons/react/24/outline';

export function Composer() {
  const { isComposing, setIsComposing, composeDraft, setComposeDraft } =
    useMailStore();
  const [to, setTo] = useState('');
  const [cc, setCc] = useState('');
  const [bcc, setBcc] = useState('');
  const [subject, setSubject] = useState('');
  const [showCc, setShowCc] = useState(false);
  const [showBcc, setShowBcc] = useState(false);
  const [attachments, setAttachments] = useState<File[]>([]);
  const [sending, setSending] = useState(false);

  const editor = useEditor({
    extensions: [
      StarterKit,
      Underline,
      Link.configure({
        openOnClick: false,
      }),
      TextAlign.configure({
        types: ['heading', 'paragraph'],
      }),
    ],
    content: '',
    editorProps: {
      attributes: {
        class:
          'prose max-w-none focus:outline-none min-h-[300px] p-4',
      },
    },
  });

  useEffect(() => {
    if (composeDraft) {
      setTo(composeDraft.to.join(', '));
      setCc(composeDraft.cc.join(', '));
      setBcc(composeDraft.bcc.join(', '));
      setSubject(composeDraft.subject);
      editor?.commands.setContent(composeDraft.body);
      setAttachments(composeDraft.attachments);
    }
  }, [composeDraft]);

  if (!isComposing) {
    return null;
  }

  async function handleSend() {
    if (!to || !editor) return;

    setSending(true);
    try {
      // Parse email addresses
      const toAddresses = to.split(',').map((email) => ({
        email: email.trim(),
      }));
      const ccAddresses = cc
        ? cc.split(',').map((email) => ({ email: email.trim() }))
        : [];
      const bccAddresses = bcc
        ? bcc.split(',').map((email) => ({ email: email.trim() }))
        : [];

      // Get user session for from address
      const session = await jmapClient.getSession();
      const fromAddress = { email: session.username };

      // Upload attachments
      const uploadedAttachments = await Promise.all(
        attachments.map(async (file) => {
          const blobId = await jmapClient.uploadBlob(file);
          return {
            blobId,
            name: file.name,
            type: file.type,
          };
        })
      );

      // Send email
      await jmapClient.sendEmail({
        from: [fromAddress],
        to: toAddresses,
        cc: ccAddresses,
        bcc: bccAddresses,
        subject: subject || '(no subject)',
        htmlBody: editor.getHTML(),
        attachments: uploadedAttachments.length > 0 ? uploadedAttachments : undefined,
      });

      // Close composer and reset
      handleClose();
    } catch (error) {
      console.error('Failed to send email:', error);
      alert('Failed to send email. Please try again.');
    } finally {
      setSending(false);
    }
  }

  function handleClose() {
    if (confirm('Discard this draft?')) {
      setIsComposing(false);
      setComposeDraft(null);
      setTo('');
      setCc('');
      setBcc('');
      setSubject('');
      setAttachments([]);
      editor?.commands.setContent('');
    }
  }

  function handleAttachFile() {
    const input = document.createElement('input');
    input.type = 'file';
    input.multiple = true;
    input.onchange = (e: any) => {
      const files = Array.from(e.target.files) as File[];
      setAttachments([...attachments, ...files]);
    };
    input.click();
  }

  function removeAttachment(index: number) {
    setAttachments(attachments.filter((_, i) => i !== index));
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black bg-opacity-50">
      <div className="bg-white rounded-lg shadow-xl w-full max-w-4xl max-h-[90vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-gray-200">
          <h2 className="text-lg font-semibold">New Message</h2>
          <button
            onClick={handleClose}
            className="p-1 hover:bg-gray-100 rounded transition-colors"
          >
            <XMarkIcon className="w-6 h-6 text-gray-600" />
          </button>
        </div>

        {/* Email fields */}
        <div className="border-b border-gray-200">
          <div className="flex items-center px-6 py-3 border-b border-gray-200">
            <label className="w-16 text-sm text-gray-600">To</label>
            <input
              type="text"
              value={to}
              onChange={(e) => setTo(e.target.value)}
              placeholder="Recipients"
              className="flex-1 outline-none text-sm"
            />
            <div className="flex gap-2 ml-4">
              <button
                onClick={() => setShowCc(!showCc)}
                className="text-sm text-blue-600 hover:text-blue-700"
              >
                Cc
              </button>
              <button
                onClick={() => setShowBcc(!showBcc)}
                className="text-sm text-blue-600 hover:text-blue-700"
              >
                Bcc
              </button>
            </div>
          </div>

          {showCc && (
            <div className="flex items-center px-6 py-3 border-b border-gray-200">
              <label className="w-16 text-sm text-gray-600">Cc</label>
              <input
                type="text"
                value={cc}
                onChange={(e) => setCc(e.target.value)}
                placeholder="Carbon copy"
                className="flex-1 outline-none text-sm"
              />
            </div>
          )}

          {showBcc && (
            <div className="flex items-center px-6 py-3 border-b border-gray-200">
              <label className="w-16 text-sm text-gray-600">Bcc</label>
              <input
                type="text"
                value={bcc}
                onChange={(e) => setBcc(e.target.value)}
                placeholder="Blind carbon copy"
                className="flex-1 outline-none text-sm"
              />
            </div>
          )}

          <div className="flex items-center px-6 py-3">
            <label className="w-16 text-sm text-gray-600">Subject</label>
            <input
              type="text"
              value={subject}
              onChange={(e) => setSubject(e.target.value)}
              placeholder="Subject"
              className="flex-1 outline-none text-sm"
            />
          </div>
        </div>

        {/* Toolbar */}
        {editor && (
          <div className="flex items-center gap-1 px-6 py-2 border-b border-gray-200">
            <button
              onClick={() => editor.chain().focus().toggleBold().run()}
              className={`p-2 rounded hover:bg-gray-100 ${
                editor.isActive('bold') ? 'bg-gray-200' : ''
              }`}
              title="Bold"
            >
              <BoldIcon className="w-4 h-4" />
            </button>
            <button
              onClick={() => editor.chain().focus().toggleItalic().run()}
              className={`p-2 rounded hover:bg-gray-100 ${
                editor.isActive('italic') ? 'bg-gray-200' : ''
              }`}
              title="Italic"
            >
              <ItalicIcon className="w-4 h-4" />
            </button>
            <button
              onClick={() => editor.chain().focus().toggleUnderline().run()}
              className={`p-2 rounded hover:bg-gray-100 ${
                editor.isActive('underline') ? 'bg-gray-200' : ''
              }`}
              title="Underline"
            >
              <UnderlineIcon className="w-4 h-4" />
            </button>

            <div className="w-px h-6 bg-gray-300 mx-2" />

            <button
              onClick={() => editor.chain().focus().toggleBulletList().run()}
              className={`p-2 rounded hover:bg-gray-100 ${
                editor.isActive('bulletList') ? 'bg-gray-200' : ''
              }`}
              title="Bullet list"
            >
              <ListBulletIcon className="w-4 h-4" />
            </button>
            <button
              onClick={() => editor.chain().focus().toggleOrderedList().run()}
              className={`p-2 rounded hover:bg-gray-100 ${
                editor.isActive('orderedList') ? 'bg-gray-200' : ''
              }`}
              title="Numbered list"
            >
              <ListNumberIcon className="w-4 h-4" />
            </button>

            <div className="w-px h-6 bg-gray-300 mx-2" />

            <button
              onClick={() => {
                const url = window.prompt('Enter URL');
                if (url) {
                  editor.chain().focus().setLink({ href: url }).run();
                }
              }}
              className="p-2 rounded hover:bg-gray-100"
              title="Insert link"
            >
              <LinkIcon className="w-4 h-4" />
            </button>
          </div>
        )}

        {/* Editor */}
        <div className="flex-1 overflow-y-auto">
          <EditorContent editor={editor} />
        </div>

        {/* Attachments */}
        {attachments.length > 0 && (
          <div className="px-6 py-3 border-t border-gray-200">
            <div className="flex flex-wrap gap-2">
              {attachments.map((file, index) => (
                <div
                  key={index}
                  className="flex items-center gap-2 px-3 py-2 bg-gray-100 rounded-lg"
                >
                  <PaperClipIcon className="w-4 h-4 text-gray-600" />
                  <span className="text-sm">{file.name}</span>
                  <button
                    onClick={() => removeAttachment(index)}
                    className="ml-2 text-gray-500 hover:text-red-600"
                  >
                    <XMarkIcon className="w-4 h-4" />
                  </button>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Footer */}
        <div className="flex items-center justify-between px-6 py-4 border-t border-gray-200">
          <div className="flex items-center gap-2">
            <button
              onClick={handleSend}
              disabled={!to || sending}
              className="flex items-center gap-2 px-4 py-2 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              <PaperAirplaneIcon className="w-4 h-4" />
              {sending ? 'Sending...' : 'Send'}
            </button>

            <button
              onClick={handleAttachFile}
              className="p-2 hover:bg-gray-100 rounded-lg transition-colors"
              title="Attach file"
            >
              <PaperClipIcon className="w-5 h-5 text-gray-600" />
            </button>
          </div>

          <button
            onClick={handleClose}
            className="text-sm text-gray-600 hover:text-gray-900"
          >
            Discard
          </button>
        </div>
      </div>
    </div>
  );
}
