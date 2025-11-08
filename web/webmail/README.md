# Nova Mail Webmail

Modern, fast, and feature-rich email client built with Next.js 14, React, and TipTap.

## Features

- **Modern UI**: Clean, responsive interface built with Tailwind CSS
- **Rich Text Editing**: Powered by TipTap for composing emails
- **Real-time Updates**: WebSocket support for instant notifications
- **Search**: Fast full-text search integration
- **Threads**: Conversation view with threading
- **Labels & Filters**: Organize emails with labels and create custom filters
- **Keyboard Shortcuts**: Vim-inspired shortcuts for power users
- **Offline Support**: Progressive Web App (PWA) capabilities

## Tech Stack

- **Framework**: Next.js 14 (App Router)
- **UI**: React 18, Tailwind CSS
- **Editor**: TipTap (rich text email composition)
- **State**: Zustand + React Query
- **API**: JMAP protocol over HTTP/3
- **Icons**: Lucide React

## Development

```bash
# Install dependencies
npm install

# Run development server
npm run dev

# Build for production
npm run build

# Start production server
npm start
```

## Environment Variables

Create a `.env.local` file:

```env
NEXT_PUBLIC_API_URL=http://localhost:8080
NEXT_PUBLIC_JMAP_URL=http://localhost:8081
```

## Project Structure

```
webmail/
├── src/
│   ├── app/              # Next.js app router pages
│   ├── components/       # React components
│   │   ├── inbox/        # Inbox view components
│   │   ├── composer/     # Email composition
│   │   ├── search/       # Search interface
│   │   └── ui/           # Shared UI components
│   ├── lib/              # Utilities and helpers
│   │   ├── jmap/         # JMAP client
│   │   ├── api/          # API clients
│   │   └── store/        # State management
│   └── styles/           # Global styles
├── public/               # Static assets
└── package.json
```

## Features Roadmap

- [x] Basic email listing
- [x] Email composition with TipTap
- [x] Search integration
- [ ] Thread view
- [ ] Labels and filters
- [ ] Snooze and scheduled send
- [ ] AI assistant integration
- [ ] Offline mode
- [ ] Mobile app (React Native)

## Performance

- Server-side rendering for fast initial load
- Incremental Static Regeneration for cached pages
- HTTP/3 for JMAP API calls
- Code splitting and lazy loading
- Image optimization

## License

Proprietary - All Rights Reserved
