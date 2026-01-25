import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { ApiClient } from './api';

describe('ApiClient', () => {
  const mockFetch = vi.fn();
  
  beforeEach(() => {
    vi.stubGlobal('fetch', mockFetch);
    mockFetch.mockReset();
  });
  
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  describe('constructor', () => {
    it('uses default base URL when no options provided', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve('{"status":"ok"}'),
      });
      
      const client = new ApiClient();
      await client.health();
      
      expect(mockFetch).toHaveBeenCalledWith(
        'http://127.0.0.1:7432/health/detailed',
        expect.any(Object)
      );
    });

    it('accepts custom base URL', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve('{"status":"ok"}'),
      });
      
      const client = new ApiClient({ baseUrl: 'http://localhost:3000' });
      await client.health();
      
      expect(mockFetch).toHaveBeenCalledWith(
        'http://localhost:3000/health/detailed',
        expect.any(Object)
      );
    });

    it('includes token in headers when provided', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve('{"status":"ok"}'),
      });
      
      const client = new ApiClient({ token: 'test-token' });
      await client.health();
      
      expect(mockFetch).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          headers: expect.objectContaining({
            'X-AgentKanban-Token': 'test-token',
          }),
        })
      );
    });
  });

  describe('configure', () => {
    it('updates baseUrl for subsequent requests', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve('{"status":"ok"}'),
      });
      
      const client = new ApiClient();
      client.configure({ baseUrl: 'http://new-host:8080' });
      await client.health();
      
      expect(mockFetch).toHaveBeenCalledWith(
        'http://new-host:8080/health/detailed',
        expect.any(Object)
      );
    });

    it('updates token for subsequent requests', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve('{"status":"ok"}'),
      });
      
      const client = new ApiClient();
      client.configure({ token: 'configured-token' });
      await client.health();
      
      expect(mockFetch).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          headers: expect.objectContaining({
            'X-AgentKanban-Token': 'configured-token',
          }),
        })
      );
    });

    it('updates both baseUrl and token', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve('{"status":"ok"}'),
      });
      
      const client = new ApiClient();
      client.configure({ baseUrl: 'http://api.example.com', token: 'api-key' });
      await client.health();
      
      expect(mockFetch).toHaveBeenCalledWith(
        'http://api.example.com/health/detailed',
        expect.objectContaining({
          headers: expect.objectContaining({
            'X-AgentKanban-Token': 'api-key',
          }),
        })
      );
    });

    it('preserves baseUrl when only token provided', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve('{"status":"ok"}'),
      });
      
      const client = new ApiClient({ baseUrl: 'http://original:1234' });
      client.configure({ token: 'new-token' });
      await client.health();
      
      expect(mockFetch).toHaveBeenCalledWith(
        'http://original:1234/health/detailed',
        expect.any(Object)
      );
    });
  });

  describe('setToken', () => {
    it('sets the authentication token for requests', async () => {
      mockFetch.mockResolvedValue({
        ok: true,
        text: () => Promise.resolve('{"status":"ok"}'),
      });
      
      const client = new ApiClient();
      client.setToken('auth-token');
      await client.health();
      
      expect(mockFetch).toHaveBeenCalledWith(
        expect.any(String),
        expect.objectContaining({
          headers: expect.objectContaining({
            'X-AgentKanban-Token': 'auth-token',
          }),
        })
      );
    });
  });
});
